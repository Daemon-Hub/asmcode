use crate::editor::{apply_completion_to_code, extract_prefix_at_offset};
use crate::ui::AppWindow;
use crate::ui::controller::IdeController;
use crate::ui::editor_bridge::{
    populate_docs_ui, record_editor_mutation, refresh_suggestions_ui, update_highlight_and_labels,
};
use crate::ui::handlers::file_ops::sync_tabs_ui;
use slint::{ComponentHandle, Model, ModelRc, SharedString, VecModel};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub fn attach_autocomplete_handlers(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
    let prev_code_len = Arc::new(std::sync::atomic::AtomicUsize::new(0));

    // Apply Completion at Cursor Offset (replaces the typed prefix seamlessly)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    let prev_len_apply_at = Arc::clone(&prev_code_len);
    app.on_apply_completion_at(move |snippet, byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let code = ui.get_code_text().to_string();
        let bytes = code.as_bytes();
        let offset = (byte_offset as usize).min(bytes.len());

        let mut word_start = offset;
        while word_start > 0 {
            let b = bytes[word_start - 1];
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'?' || b == b'@' {
                word_start -= 1;
            } else {
                break;
            }
        }

        let mut word_end = offset;
        while word_end < bytes.len() {
            let b = bytes[word_end];
            if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'?' || b == b'@' {
                word_end += 1;
            } else {
                break;
            }
        }

        let mut new_code = String::with_capacity(code.len() + snippet.len());
        new_code.push_str(&code[..word_start]);
        new_code.push_str(snippet.as_str());
        let new_cursor_pos = new_code.len();
        new_code.push_str(&code[word_end..]);

        prev_len_apply_at.store(new_code.len(), std::sync::atomic::Ordering::Relaxed);
        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);
        ui.set_show_autocomplete_popup(false);

        let ctrl = ctrl_clone.lock().unwrap();
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        new_cursor_pos as i32
    });

    // Code Edited (Typing in Editor with Live Autocomplete Trigger)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    let prev_len_clone = Arc::clone(&prev_code_len);
    app.on_code_edited(move |new_code, cursor_offset| {
        let Some(ui) = app_weak.upgrade() else { return };
        ui.set_is_dirty(true);

        {
            let mut ctrl = ctrl_clone.lock().unwrap();
            record_editor_mutation(&ui, &mut ctrl, new_code.as_str(), cursor_offset, false);
            sync_tabs_ui(&ui, &ctrl);
        }

        let is_asm = ui.get_is_active_asm();
        {
            let mut ctrl = ctrl_clone.lock().unwrap();
            update_highlight_and_labels(&ui, &ctrl.autocomplete, new_code.as_str(), is_asm);

            if ui.get_show_find_replace() {
                let query = ctrl.find_state.query.clone();
                if !query.is_empty() {
                    let match_case = ctrl.find_state.match_case;
                    let whole_word = ctrl.find_state.whole_word;
                    let matches = crate::editor::find_all_matches(new_code.as_str(), &query, match_case, whole_word);
                    let count = matches.len();
                    ctrl.find_state.matches = matches;
                    if count > 0 {
                        ctrl.find_state.current_idx = ctrl.find_state.current_idx.min(count - 1);
                        ui.set_find_match_count(count as i32);
                        ui.set_find_match_index((ctrl.find_state.current_idx + 1) as i32);
                    } else {
                        ctrl.find_state.current_idx = 0;
                        ui.set_find_match_count(0);
                        ui.set_find_match_index(0);
                    }
                    crate::ui::handlers::editor_ops::sync_find_matches_to_ui(&ui, &ctrl, new_code.as_str());
                }
            }
        }

        let is_large_file = ui.get_is_large_file_mode();
        if is_large_file {
            ui.set_show_autocomplete_popup(false);
            return;
        }

        let old_len = prev_len_clone.swap(new_code.len(), std::sync::atomic::Ordering::Relaxed);
        let was_popup_open = ui.get_show_autocomplete_popup();
        let is_insertion = new_code.len() > old_len;

        // Autocomplete must only trigger on interactive typing of a single character.
        // Pasting multiple characters or batch insertions must never open the autocomplete popup!
        let diff = new_code.len().saturating_sub(old_len);
        let is_single_char_type = if diff == 1 {
            true
        } else if diff > 1 && diff <= 4 && cursor_offset >= diff as i32 {
            new_code
                .get((cursor_offset as usize - diff)..cursor_offset as usize)
                .map(|s| s.chars().count() == 1)
                .unwrap_or(false)
        } else {
            false
        };

        let lang = ui.get_current_language().to_string();
        let prefix = extract_prefix_at_offset(new_code.as_str(), cursor_offset as usize);

        // Only auto-trigger autocomplete when actively typing/inserting single identifier characters,
        // or if the popup was already open and user is refining completion.
        // Deleting lines or characters when popup was closed must never open the popup!
        let should_trigger = if is_insertion && is_single_char_type {
            let b = if cursor_offset > 0 && (cursor_offset as usize) <= new_code.len() {
                new_code.as_bytes().get(cursor_offset as usize - 1).copied()
            } else {
                None
            };
            match b {
                Some(ch) => ch.is_ascii_alphanumeric() || ch == b'_' || ch == b'@',
                None => false,
            }
        } else {
            !is_insertion && was_popup_open
        };

        if is_asm && !prefix.is_empty() && should_trigger {
            refresh_suggestions_ui(&ui, &ctrl_clone, prefix, &lang);
            let has_suggestions = ui.get_suggestion_list().row_count() > 0;
            if has_suggestions {
                ui.set_popup_anchor_x(ui.get_cursor_x());
                ui.set_popup_anchor_y(ui.get_cursor_y());
                let cy = ui.get_cursor_y();
                let line_h = ui.get_editor_line_height();
                let line_idx = if line_h > 0.0 {
                    (cy / line_h).round() as i32
                } else {
                    0
                };
                ui.set_popup_trigger_line(line_idx);
                ui.set_show_autocomplete_popup(true);
            } else {
                ui.set_show_autocomplete_popup(false);
            }
        } else {
            ui.set_show_autocomplete_popup(false);
        }
    });

    // Trigger Autocomplete (Ctrl + Space)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_trigger_autocomplete(move |cursor_offset| {
        let Some(ui) = app_weak.upgrade() else { return };
        if ui.get_is_large_file_mode() {
            ui.set_show_autocomplete_popup(false);
            return;
        }
        let code = ui.get_code_text().to_string();
        let prefix = extract_prefix_at_offset(&code, cursor_offset as usize);
        let lang = ui.get_current_language().to_string();

        ui.set_popup_anchor_x(ui.get_cursor_x());
        ui.set_popup_anchor_y(ui.get_cursor_y());
        let cy = ui.get_cursor_y();
        let line_h = ui.get_editor_line_height();
        let line_idx = if line_h > 0.0 {
            (cy / line_h).round() as i32
        } else {
            0
        };
        ui.set_popup_trigger_line(line_idx);

        refresh_suggestions_ui(&ui, &ctrl_clone, prefix, &lang);
        ui.set_show_autocomplete_popup(true);
    });

    // Apply In-place Completion (from list click)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    let prev_len_apply = Arc::clone(&prev_code_len);
    app.on_apply_completion(move |snippet| {
        let Some(ui) = app_weak.upgrade() else { return };
        let code = ui.get_code_text().to_string();
        let cy = ui.get_cursor_y();
        let line_h = ui.get_editor_line_height();
        let line_idx = if line_h > 0.0 {
            (cy / line_h).round() as usize
        } else {
            0
        };
        let updated_code = apply_completion_to_code(&code, line_idx, snippet.as_str());
        prev_len_apply.store(updated_code.len(), std::sync::atomic::Ordering::Relaxed);
        ui.set_code_text(SharedString::from(&updated_code));
        ui.set_is_dirty(true);
        ui.set_show_autocomplete_popup(false);

        let ctrl = ctrl_clone.lock().unwrap();
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &updated_code, is_asm);
    });

    // Navigate Suggestions (Up/Down Arrow in Popup)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_navigate_suggestion(move |delta| {
        let Some(ui) = app_weak.upgrade() else { return };
        let ctrl = ctrl_clone.lock().unwrap();
        if ctrl.active_suggestions.is_empty() {
            return;
        }
        let current_name = ui.get_selected_sugg_name();
        let current_idx = ctrl
            .active_suggestions
            .iter()
            .position(|s| s.name == current_name)
            .unwrap_or(0);
        let len = ctrl.active_suggestions.len();
        let next_idx = if delta > 0 {
            (current_idx + 1) % len
        } else {
            (current_idx + len - 1) % len
        };
        if let Some(sugg) = ctrl.active_suggestions.get(next_idx) {
            ui.set_selected_sugg_name(sugg.name.clone());
            ui.set_selected_sugg_category(sugg.category.clone());
            ui.set_selected_sugg_syntax(sugg.syntax.clone());
            ui.set_selected_sugg_summary(sugg.summary.clone());
            ui.set_selected_sugg_flags(sugg.flags.clone());
            ui.set_selected_sugg_cycles(sugg.cycles.clone());
            ui.set_selected_sugg_insert_text(sugg.insert_text.clone());
            ui.set_selected_sugg_icon_kind(sugg.icon_kind.clone());
            ui.set_selected_sugg_example(sugg.example.clone());
        }
    });

    // Toggle Language (RU / EN)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_toggle_language(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let cur = ui.get_current_language().to_string();
        let next_lang = if cur == "ru" { "en" } else { "ru" };
        ui.set_current_language(SharedString::from(next_lang));
        populate_docs_ui(&ui, next_lang);
        refresh_suggestions_ui(&ui, &ctrl_clone, ui.get_search_filter().as_str(), next_lang);
        if next_lang == "ru" {
            ui.set_status_message(SharedString::from("Язык изменён на Русский"));
        } else {
            ui.set_status_message(SharedString::from("Language set to English"));
        }
    });

    // Filter Changed in Autocomplete Drawer
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_filter_changed(move |query| {
        let Some(ui) = app_weak.upgrade() else { return };
        let lang = ui.get_current_language().to_string();
        let q = query.to_string();

        // 1. Filter suggestions (Мнемоники)
        refresh_suggestions_ui(&ui, &ctrl_clone, &q, &lang);

        // 2. Filter BDOS calls (Вызовы BDOS)
        let is_ru = lang == "ru";
        let all_label = if is_ru { "Все" } else { "All" };
        crate::ui::docs_bridge::populate_bdos_list(&ui, &lang, all_label, &q);

        // 3. Filter ASCII table (Таблица ASCII)
        let q_lower = q.to_lowercase();
        let filtered_ascii: Vec<crate::ui::AsciiItem> = crate::docs::ascii_table::ASCII_TABLE
            .iter()
            .filter(|a| {
                q_lower.is_empty()
                    || a.code.to_string().contains(&q_lower)
                    || a.hex.to_lowercase().contains(&q_lower)
                    || a.char_display.to_lowercase().contains(&q_lower)
                    || a.name_ru.to_lowercase().contains(&q_lower)
                    || a.name_en.to_lowercase().contains(&q_lower)
                    || a.desc_ru.to_lowercase().contains(&q_lower)
                    || a.desc_en.to_lowercase().contains(&q_lower)
            })
            .map(|a| crate::ui::AsciiItem {
                code_dec: a.code as i32,
                code_hex: SharedString::from(a.hex),
                char_display: SharedString::from(a.char_display),
                name: SharedString::from(if is_ru { a.name_ru } else { a.name_en }),
                description: SharedString::from(if is_ru { a.desc_ru } else { a.desc_en }),
                is_cpm_special: a.is_cpm_special,
            })
            .collect();
        ui.set_ascii_list(ModelRc::from(Rc::new(VecModel::from(filtered_ascii))));
    });

    // Select Suggestion
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_select_suggestion(move |idx| {
        let Some(ui) = app_weak.upgrade() else { return };
        let ctrl = ctrl_clone.lock().unwrap();
        if let Some(sugg) = ctrl.active_suggestions.get(idx as usize) {
            ui.set_selected_sugg_name(sugg.name.clone());
            ui.set_selected_sugg_category(sugg.category.clone());
            ui.set_selected_sugg_syntax(sugg.syntax.clone());
            ui.set_selected_sugg_summary(sugg.summary.clone());
            ui.set_selected_sugg_flags(sugg.flags.clone());
            ui.set_selected_sugg_cycles(sugg.cycles.clone());
            ui.set_selected_sugg_insert_text(sugg.insert_text.clone());
            ui.set_selected_sugg_icon_kind(sugg.icon_kind.clone());
            ui.set_selected_sugg_example(sugg.example.clone());
        }
    });

    // Insert Suggestion Template
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    let prev_len_insert = Arc::clone(&prev_code_len);
    app.on_insert_suggestion(move |snippet| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut current_code = ui.get_code_text().to_string();
        if !current_code.is_empty() && !current_code.ends_with('\n') {
            current_code.push('\n');
        }
        for line in snippet.lines() {
            if !line.starts_with(' ') && !line.starts_with('\t') && !line.ends_with(':') {
                current_code.push_str("        ");
            }
            current_code.push_str(line);
            current_code.push('\n');
        }

        prev_len_insert.store(current_code.len(), std::sync::atomic::Ordering::Relaxed);
        ui.set_code_text(SharedString::from(&current_code));
        ui.set_is_dirty(true);
        ui.set_show_autocomplete_popup(false);

        let ctrl = ctrl_clone.lock().unwrap();
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &current_code, is_asm);
    });
}
