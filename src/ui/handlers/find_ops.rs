use crate::editor::{find_all_matches, replace_all_matches, replace_current_match};
use crate::syntax::intel8080::build_line_starts;
use crate::ui::AppWindow;
use crate::ui::controller::IdeController;
use crate::ui::editor_bridge::{record_editor_mutation, update_highlight_and_labels};
use crate::ui::handlers::tab_ops::sync_tabs_ui;
use rayon::prelude::*;
use slint::{Color, ComponentHandle, ModelRc, SharedString, VecModel};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

const MAX_RENDERED_FIND_MATCHES: usize = 500;

#[cfg(test)]
pub fn compute_find_match_items(
    code: &str,
    matches: &[(usize, usize)],
    current_idx: usize,
) -> Vec<crate::ui::FindMatchItem> {
    compute_find_match_items_with_starts(code, None, matches, current_idx)
}

pub fn compute_find_match_items_with_starts(
    code: &str,
    cached_line_starts: Option<&[usize]>,
    matches: &[(usize, usize)],
    current_idx: usize,
) -> Vec<crate::ui::FindMatchItem> {
    if matches.is_empty() {
        return Vec::new();
    }

    let owned_starts;
    let line_starts: &[usize] = match cached_line_starts {
        Some(s) if !s.is_empty() => s,
        _ => {
            owned_starts = build_line_starts(code);
            &owned_starts
        }
    };

    // Window around current_idx when there are thousands of matches so Slint never lays out 10,000+ rectangles
    let (slice_start, slice_end) = if matches.len() <= MAX_RENDERED_FIND_MATCHES {
        (0, matches.len())
    } else {
        let half = MAX_RENDERED_FIND_MATCHES / 2;
        let start = current_idx
            .saturating_sub(half)
            .min(matches.len() - MAX_RENDERED_FIND_MATCHES);
        (start, start + MAX_RENDERED_FIND_MATCHES)
    };

    let build_item = |m_idx: usize, &(start_byte, end_byte): &(usize, usize)| {
        let line_num = match line_starts.binary_search(&start_byte) {
            Ok(idx) => idx,
            Err(idx) => idx.saturating_sub(1),
        };

        let line_start = line_starts[line_num];
        let char_offset = if start_byte > line_start && start_byte <= code.len() {
            code[line_start..start_byte].chars().count()
        } else {
            0
        };

        let char_len = if end_byte > start_byte && end_byte <= code.len() {
            code[start_byte..end_byte].chars().count()
        } else {
            1
        };

        crate::ui::FindMatchItem {
            line_number: line_num as i32,
            char_offset: char_offset as i32,
            char_length: char_len as i32,
            is_current: m_idx == current_idx,
        }
    };

    let visible = &matches[slice_start..slice_end];
    if visible.len() >= 128 {
        visible
            .par_iter()
            .enumerate()
            .map(|(offset, pair)| build_item(slice_start + offset, pair))
            .collect()
    } else {
        visible
            .iter()
            .enumerate()
            .map(|(offset, pair)| build_item(slice_start + offset, pair))
            .collect()
    }
}

pub fn sync_find_matches_to_ui(ui: &AppWindow, ctrl: &IdeController, code: &str) {
    let line_starts = ctrl
        .file_manager
        .active_file_path
        .as_ref()
        .and_then(|p| ctrl.tab_line_starts.get(p))
        .map(Vec::as_slice);
    let items = compute_find_match_items_with_starts(
        code,
        line_starts,
        &ctrl.find_state.matches,
        ctrl.find_state.current_idx,
    );
    ui.set_find_matches(ModelRc::from(Rc::new(VecModel::from(items))));
}

fn step_find_match(ui: &AppWindow, ctrl_rc: &Arc<Mutex<IdeController>>, forward: bool) {
    let code = ui.get_code_text().to_string();
    let mut ctrl = ctrl_rc.lock().unwrap();
    let count = ctrl.find_state.matches.len();
    if count == 0 {
        return;
    }

    ctrl.find_state.current_idx = if forward {
        (ctrl.find_state.current_idx + 1) % count
    } else {
        (ctrl.find_state.current_idx + count - 1) % count
    };
    let idx = ctrl.find_state.current_idx;
    let range = ctrl.find_state.matches[idx];

    ui.set_find_match_index((idx + 1) as i32);
    ui.set_select_range_start(-1);
    ui.set_select_range_start(range.0 as i32);
    ui.set_select_range_end(range.1 as i32);
    sync_find_matches_to_ui(ui, &ctrl, &code);
}

pub fn attach_find_handlers(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
    // Open Find/Replace with optional selection pre-fill (Ctrl+F / Ctrl+R)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_request_open_find(move |sel_start, sel_end, is_replace| {
        let Some(ui) = app_weak.upgrade() else { return };
        let code = ui.get_code_text().to_string();
        let mut ctrl = ctrl_clone.lock().unwrap();

        let s = (sel_start.max(0) as usize).min(code.len());
        let e = (sel_end.max(0) as usize).min(code.len());

        let mut query_to_run = ctrl.find_state.query.clone();

        if s < e {
            let selected = &code[s..e];
            if !selected.is_empty() && !selected.contains('\n') && selected.len() <= 200 {
                query_to_run = selected.to_string();
                ui.set_find_text(SharedString::from(selected));
                ctrl.find_state.query = query_to_run.clone();
            }
        }

        ui.set_show_find_replace(true);
        ui.set_show_replace_row(is_replace);

        if !query_to_run.is_empty() {
            let match_case = ctrl.find_state.match_case;
            let whole_word = ctrl.find_state.whole_word;
            let matches = find_all_matches(&code, &query_to_run, match_case, whole_word);
            let count = matches.len();
            ctrl.find_state.matches = matches;

            if count > 0 {
                let target_idx = ctrl
                    .find_state
                    .matches
                    .iter()
                    .position(|&(ms, _)| ms >= s)
                    .unwrap_or(0);
                ctrl.find_state.current_idx = target_idx;
                let range = ctrl.find_state.matches[target_idx];
                ui.set_find_match_count(count as i32);
                ui.set_find_match_index((target_idx + 1) as i32);
                ui.set_select_range_start(-1);
                ui.set_select_range_start(range.0 as i32);
                ui.set_select_range_end(range.1 as i32);
            } else {
                ctrl.find_state.current_idx = 0;
                ui.set_find_match_count(0);
                ui.set_find_match_index(0);
            }
            sync_find_matches_to_ui(&ui, &ctrl, &code);
        } else {
            ui.set_find_matches(ModelRc::from(Rc::new(VecModel::from(Vec::new()))));
        }
    });

    // Editor Find
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_editor_find(move |query, match_case, whole_word| {
        let Some(ui) = app_weak.upgrade() else { return };
        let code = ui.get_code_text().to_string();
        let q = query.to_string();

        let matches = find_all_matches(&code, &q, match_case, whole_word);
        let count = matches.len();

        let mut ctrl = ctrl_clone.lock().unwrap();
        ctrl.find_state.query = q;
        ctrl.find_state.match_case = match_case;
        ctrl.find_state.whole_word = whole_word;
        ctrl.find_state.matches = matches;

        if count == 0 {
            ctrl.find_state.current_idx = 0;
            ui.set_find_match_count(0);
            ui.set_find_match_index(0);
            ui.set_find_matches(ModelRc::from(Rc::new(VecModel::from(Vec::new()))));
            return;
        }

        let target_idx = 0;
        ctrl.find_state.current_idx = target_idx;
        let range = ctrl.find_state.matches[target_idx];

        ui.set_find_match_count(count as i32);
        ui.set_find_match_index(1);
        ui.set_select_range_start(-1);
        ui.set_select_range_start(range.0 as i32);
        ui.set_select_range_end(range.1 as i32);
        sync_find_matches_to_ui(&ui, &ctrl, &code);
    });

    // Editor Find Next
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_editor_find_next(move || {
        if let Some(ui) = app_weak.upgrade() {
            step_find_match(&ui, &ctrl_clone, true);
        }
    });

    // Editor Find Prev
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_editor_find_prev(move || {
        if let Some(ui) = app_weak.upgrade() {
            step_find_match(&ui, &ctrl_clone, false);
        }
    });

    // Editor Replace Current Match
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_editor_replace(move |replace_with| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        let count = ctrl.find_state.matches.len();
        if count == 0 {
            return;
        }

        let idx = ctrl.find_state.current_idx.min(count - 1);
        let range = ctrl.find_state.matches[idx];
        let code = ui.get_code_text().to_string();

        let (new_code, new_pos) = replace_current_match(&code, range, replace_with.as_str());
        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        record_editor_mutation(&ui, &mut ctrl, &new_code, new_pos as i32, false);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        let query = ctrl.find_state.query.clone();
        let match_case = ctrl.find_state.match_case;
        let whole_word = ctrl.find_state.whole_word;
        let new_matches = find_all_matches(&new_code, &query, match_case, whole_word);
        let new_count = new_matches.len();
        ctrl.find_state.matches = new_matches;

        if new_count == 0 {
            ctrl.find_state.current_idx = 0;
            ui.set_find_match_count(0);
            ui.set_find_match_index(0);
            ui.set_find_matches(ModelRc::from(Rc::new(VecModel::from(Vec::new()))));
        } else {
            let next_idx = idx % new_count;
            ctrl.find_state.current_idx = next_idx;
            let next_range = ctrl.find_state.matches[next_idx];
            ui.set_find_match_count(new_count as i32);
            ui.set_find_match_index((next_idx + 1) as i32);
            ui.set_select_range_start(-1);
            ui.set_select_range_start(next_range.0 as i32);
            ui.set_select_range_end(next_range.1 as i32);
            sync_find_matches_to_ui(&ui, &ctrl, &new_code);
        }
    });

    // Editor Replace All Matches
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_editor_replace_all(move |replace_with| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        if ctrl.find_state.matches.is_empty() {
            return;
        }

        let code = ui.get_code_text().to_string();
        let (new_code, replaced_count) =
            replace_all_matches(&code, &ctrl.find_state.matches, replace_with.as_str());

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        record_editor_mutation(&ui, &mut ctrl, &new_code, 0, false);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        ctrl.find_state.matches.clear();
        ctrl.find_state.current_idx = 0;
        ui.set_find_match_count(0);
        ui.set_find_match_index(0);
        ui.set_find_matches(ModelRc::from(Rc::new(VecModel::from(Vec::new()))));

        let lang = ui.get_current_language().to_string();
        let msg = if lang == "ru" {
            format!("Заменено {} совпадений", replaced_count)
        } else {
            format!("Replaced {} occurrences", replaced_count)
        };
        ui.set_status_message(SharedString::from(msg));
        ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_find_match_items() {
        let code = "MOV A, B\nMOV C, D\nMVI A, 42h";
        let matches = vec![(0, 3), (9, 12), (18, 21)];
        let items = compute_find_match_items(code, &matches, 1);

        assert_eq!(items.len(), 3);
        assert_eq!(items[0].line_number, 0);
        assert_eq!(items[0].char_offset, 0);
        assert_eq!(items[0].char_length, 3);
        assert!(!items[0].is_current);

        assert_eq!(items[1].line_number, 1);
        assert_eq!(items[1].char_offset, 0);
        assert_eq!(items[1].char_length, 3);
        assert!(items[1].is_current);

        assert_eq!(items[2].line_number, 2);
        assert_eq!(items[2].char_offset, 0);
        assert_eq!(items[2].char_length, 3);
        assert!(!items[2].is_current);
    }
}
