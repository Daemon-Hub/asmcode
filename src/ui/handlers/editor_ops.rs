use crate::editor::{
    delete_word_left, find_all_matches, replace_all_matches, replace_current_match,
    toggle_comment_in_code,
};
use crate::ui::AppWindow;
use crate::ui::controller::IdeController;
use crate::ui::editor_bridge::{record_editor_mutation, update_highlight_and_labels};
use crate::ui::handlers::file_ops::sync_tabs_ui;
use slint::{Color, ComponentHandle, ModelRc, SharedString, VecModel};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub fn compute_find_match_items(
    code: &str,
    matches: &[(usize, usize)],
    current_idx: usize,
) -> Vec<crate::ui::FindMatchItem> {
    if matches.is_empty() {
        return Vec::new();
    }

    let mut line_starts = Vec::new();
    line_starts.push(0);
    for (idx, b) in code.bytes().enumerate() {
        if b == b'\n' {
            line_starts.push(idx + 1);
        }
    }

    let mut items = Vec::with_capacity(matches.len());
    for (m_idx, &(start_byte, end_byte)) in matches.iter().enumerate() {
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

        items.push(crate::ui::FindMatchItem {
            line_number: line_num as i32,
            char_offset: char_offset as i32,
            char_length: char_len as i32,
            is_current: m_idx == current_idx,
        });
    }

    items
}

pub fn sync_find_matches_to_ui(ui: &AppWindow, ctrl: &IdeController, code: &str) {
    let items = compute_find_match_items(code, &ctrl.find_state.matches, ctrl.find_state.current_idx);
    ui.set_find_matches(ModelRc::from(Rc::new(VecModel::from(items))));
}

pub fn attach_editor_handlers(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
    // Viewport Scrolled (Virtualization: slice visible highlight lines)
    let app_weak = app.as_weak();
    app.on_editor_viewport_scrolled(move |scroll_line, visible_count| {
        let Some(ui) = app_weak.upgrade() else { return };
        let is_asm = ui.get_is_active_asm();
        let code = ui.get_code_text().to_string();
        crate::ui::editor_bridge::update_viewport_highlight(
            &ui,
            &code,
            scroll_line.max(0) as usize,
            visible_count.max(40) as usize,
            is_asm,
        );
    });

    // Insert Tab (Inserts 4 spaces without losing focus)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_insert_tab(move |byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let code = ui.get_code_text().to_string();
        let offset = (byte_offset as usize).min(code.len());

        let tab_spaces = "    ";
        let mut new_code = String::with_capacity(code.len() + tab_spaces.len());
        new_code.push_str(&code[..offset]);
        new_code.push_str(tab_spaces);
        new_code.push_str(&code[offset..]);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let final_pos = (offset + tab_spaces.len()) as i32;
        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, final_pos, false);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        final_pos
    });

    // Insert Newline (Preserving leading indentation / Auto-indent)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_insert_newline(move |byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let code = ui.get_code_text().to_string();
        let offset = (byte_offset as usize).min(code.len());

        let pre = &code[..offset];
        let line_start = pre.rfind('\n').map(|p| p + 1).unwrap_or(0);
        let current_line_so_far = &pre[line_start..];

        let ws_len = current_line_so_far
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        let leading_ws = &current_line_so_far[..ws_len];

        let mut new_code = String::with_capacity(code.len() + 1 + ws_len);
        new_code.push_str(&code[..offset]);
        new_code.push('\n');
        new_code.push_str(leading_ws);
        new_code.push_str(&code[offset..]);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let final_pos = (offset + 1 + ws_len) as i32;
        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, final_pos, false);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        final_pos
    });

    // Delete Word Left (Ctrl + Backspace)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_delete_word_left(move |byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let code = ui.get_code_text().to_string();
        let (new_code, final_from) = delete_word_left(&code, byte_offset as usize);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, final_from as i32, false);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        final_from as i32
    });

    // Undo (Ctrl + Z)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_editor_undo(move |current_cursor| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let mut ctrl = ctrl_clone.lock().unwrap();
        if let Some(snap) = ctrl.undo_manager.undo() {
            ui.set_code_text(SharedString::from(&snap.text));
            ui.set_is_dirty(true);
            if let Some(path) = ctrl.file_manager.active_file_path.clone() {
                ctrl.tab_buffers.insert(path, snap.text.clone());
            }
            sync_tabs_ui(&ui, &ctrl);
            let is_asm = ui.get_is_active_asm();
            update_highlight_and_labels(&ui, &ctrl.autocomplete, &snap.text, is_asm);
            if snap.cursor_offset > 0 {
                snap.cursor_offset.min(snap.text.len() as i32)
            } else {
                current_cursor.min(snap.text.len() as i32)
            }
        } else {
            current_cursor
        }
    });

    // Redo (Ctrl + Y or Ctrl + Shift + Z)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_editor_redo(move |current_cursor| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let mut ctrl = ctrl_clone.lock().unwrap();
        if let Some(snap) = ctrl.undo_manager.redo() {
            ui.set_code_text(SharedString::from(&snap.text));
            ui.set_is_dirty(true);
            if let Some(path) = ctrl.file_manager.active_file_path.clone() {
                ctrl.tab_buffers.insert(path, snap.text.clone());
            }
            sync_tabs_ui(&ui, &ctrl);
            let is_asm = ui.get_is_active_asm();
            update_highlight_and_labels(&ui, &ctrl.autocomplete, &snap.text, is_asm);
            if snap.cursor_offset > 0 {
                snap.cursor_offset.min(snap.text.len() as i32)
            } else {
                current_cursor.min(snap.text.len() as i32)
            }
        } else {
            current_cursor
        }
    });

    // Toggle Comment Range (Ctrl + /)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_toggle_comment_range(move |cursor_offset, anchor_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let code = ui.get_code_text().to_string();
        let (new_code, new_pos) =
            toggle_comment_in_code(&code, cursor_offset as usize, anchor_offset as usize);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, new_pos, true);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        new_pos
    });

    // Legacy single offset fallback
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_toggle_comment(move |byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let code = ui.get_code_text().to_string();
        let (new_code, new_pos) =
            toggle_comment_in_code(&code, byte_offset as usize, byte_offset as usize);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, new_pos, true);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        new_pos
    });

    // Duplicate Line (Ctrl + D)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_duplicate_line(move |byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let code = ui.get_code_text().to_string();
        let offset = (byte_offset as usize).min(code.len());
        let pre = &code[..offset];
        let line_start = pre.rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line_end = code[offset..]
            .find('\n')
            .map(|p| offset + p)
            .unwrap_or(code.len());
        let current_line = &code[line_start..line_end];

        let col = offset - line_start;
        let duplicate_chunk = format!("\n{}", current_line);

        let mut new_code = String::with_capacity(code.len() + duplicate_chunk.len());
        new_code.push_str(&code[..line_end]);
        new_code.push_str(&duplicate_chunk);
        new_code.push_str(&code[line_end..]);

        let new_pos = (line_end + 1 + col) as i32;
        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, new_pos, true);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        new_pos
    });

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

    // Editor Find (Live search with match count, highlighting and nearest match selection)
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

    // Editor Find Next (Cycle forward)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_editor_find_next(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let code = ui.get_code_text().to_string();
        let mut ctrl = ctrl_clone.lock().unwrap();
        let count = ctrl.find_state.matches.len();
        if count == 0 {
            return;
        }

        ctrl.find_state.current_idx = (ctrl.find_state.current_idx + 1) % count;
        let idx = ctrl.find_state.current_idx;
        let range = ctrl.find_state.matches[idx];

        ui.set_find_match_index((idx + 1) as i32);
        ui.set_select_range_start(-1);
        ui.set_select_range_start(range.0 as i32);
        ui.set_select_range_end(range.1 as i32);
        sync_find_matches_to_ui(&ui, &ctrl, &code);
    });

    // Editor Find Prev (Cycle backward)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_editor_find_prev(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let code = ui.get_code_text().to_string();
        let mut ctrl = ctrl_clone.lock().unwrap();
        let count = ctrl.find_state.matches.len();
        if count == 0 {
            return;
        }

        ctrl.find_state.current_idx = (ctrl.find_state.current_idx + count - 1) % count;
        let idx = ctrl.find_state.current_idx;
        let range = ctrl.find_state.matches[idx];

        ui.set_find_match_index((idx + 1) as i32);
        ui.set_select_range_start(-1);
        ui.set_select_range_start(range.0 as i32);
        ui.set_select_range_end(range.1 as i32);
        sync_find_matches_to_ui(&ui, &ctrl, &code);
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

        // Re-run find on updated text
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

        // First match on line 0
        assert_eq!(items[0].line_number, 0);
        assert_eq!(items[0].char_offset, 0);
        assert_eq!(items[0].char_length, 3);
        assert!(!items[0].is_current);

        // Second match on line 1 (is_current = true)
        assert_eq!(items[1].line_number, 1);
        assert_eq!(items[1].char_offset, 0);
        assert_eq!(items[1].char_length, 3);
        assert!(items[1].is_current);

        // Third match on line 2
        assert_eq!(items[2].line_number, 2);
        assert_eq!(items[2].char_offset, 0);
        assert_eq!(items[2].char_length, 3);
        assert!(!items[2].is_current);
    }
}
