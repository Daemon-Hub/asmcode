use crate::ui::AppWindow;
use crate::ui::controller::{IdeController, persist_current_session};
use crate::ui::editor_bridge::{
    refresh_active_editor_highlight_and_labels, reset_editor_ui_caches, update_highlight_and_labels,
};
use crate::ui::handlers::tab_ops::sync_tabs_ui;
use crate::ui::terminal::sync_logs;
use slint::{Color, ComponentHandle, SharedString};
use std::sync::{Arc, Mutex};

pub fn attach_view_handlers(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
    // Toggle Docs Mode
    let app_weak = app.as_weak();
    app.on_toggle_docs_mode(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let current = ui.get_editor_mode();
        if current == 2 {
            ui.set_editor_mode(0);
        } else {
            ui.set_editor_mode(2);
        }
        ui.set_term_focused(false);
        ui.invoke_term_focus_changed(false);
    });

    // Switch Editor Theme (0 = Catppuccin Dark, 1 = Cyberpunk 2077 Red)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_switch_theme(move |new_theme_id| {
        let Some(ui) = app_weak.upgrade() else { return };
        ui.set_active_theme_id(new_theme_id);

        let is_asm = ui.get_is_active_asm();
        {
            let mut ctrl = ctrl_clone.lock().unwrap();
            let theme_name = if new_theme_id == 1 {
                "Cyberpunk 2077 Red"
            } else {
                "Catppuccin Dark"
            };
            ctrl.append_log(&format!("[THEME] Active theme: {}", theme_name));
            ctrl.invalidate_viewport_cache();
            reset_editor_ui_caches();
            refresh_active_editor_highlight_and_labels(&ui, &ctrl, is_asm, true);
            let lang = ui.get_current_language().to_string();
            crate::ui::editor_bridge::populate_docs_ui(&ui, &lang);
            persist_current_session(&ui, &ctrl);
            sync_logs(&ui, &ctrl);
        }
    });

    // Label Clicked in Outline
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_label_clicked(move |line_number| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        ctrl.append_log(&format!("[OUTLINE] Jump target at line {}", line_number));
        ui.set_status_message(SharedString::from(format!("Label at line {}", line_number)));
        ui.set_status_color(Color::from_rgb_u8(249, 226, 175));
        sync_logs(&ui, &ctrl);
    });

    // Filter Docs Category
    let app_weak = app.as_weak();
    app.on_filter_docs_category(move |category| {
        let Some(ui) = app_weak.upgrade() else { return };
        let lang = ui.get_current_language().to_string();
        let query = ui.get_docs_search_query().to_string();
        crate::ui::editor_bridge::populate_filtered_docs_ui(&ui, &lang, &category, query.as_str());
    });

    // Filter BDOS Category
    let app_weak = app.as_weak();
    app.on_filter_bdos_category(move |category| {
        let Some(ui) = app_weak.upgrade() else { return };
        let lang = ui.get_current_language().to_string();
        let query = ui.get_docs_search_query().to_string();
        crate::ui::docs_bridge::populate_bdos_list(&ui, &lang, &category, query.as_str());
    });

    // Search All Docs across all modules
    let app_weak = app.as_weak();
    app.on_search_docs(move |query| {
        let Some(ui) = app_weak.upgrade() else { return };
        let lang = ui.get_current_language().to_string();
        crate::ui::docs_bridge::search_all_docs(&ui, &lang, query.as_str());
    });

    // Copy Text / Hex Code to Clipboard
    let app_weak = app.as_weak();
    app.on_copy_text_to_clipboard(move |text| {
        let Some(ui) = app_weak.upgrade() else { return };
        let lang = ui.get_current_language().to_string();
        if let Ok(mut cb) = arboard::Clipboard::new() {
            let _ = cb.set_text(text.as_str());
        }
        let msg = if lang == "ru" {
            format!("Скопировано в буфер обмена: {}", text)
        } else {
            format!("Copied to clipboard: {}", text)
        };
        ui.set_status_message(SharedString::from(msg));
    });

    // Insert Doc Snippet into Active Editor Code
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_insert_doc_snippet(move |snippet| {
        let Some(ui) = app_weak.upgrade() else { return };
        if ui.get_window_read_only() {
            return;
        }
        let mut ctrl = ctrl_clone.lock().unwrap();
        if let Some(win) = ctrl.large_window.clone() {
            if let Some(buf) = ctrl.tab_buffers.get_mut(&win.path) {
                if !buf.is_empty() && !buf.ends_with('\n') {
                    buf.push('\n');
                }
                buf.push_str(&snippet);
                buf.push('\n');
                let starts = crate::syntax::intel8080::build_line_starts(buf);
                let total = starts.len().max(1);
                let (win_text, new_win, _) = crate::ui::controller::build_window_slice_for_path(
                    win.path.clone(),
                    buf,
                    &starts,
                    total.saturating_sub(1),
                );
                ctrl.tab_line_starts.insert(win.path.clone(), starts);
                ctrl.dirty_tabs.insert(win.path.clone());
                ctrl.large_window = Some(new_win.clone());
                ui.set_window_read_only(new_win.read_only);
                ui.set_total_line_count(total as i32);
                ui.set_text_window_start_line(new_win.start_line as i32);
                ui.set_code_text(SharedString::from(win_text));
                ui.set_is_dirty(true);
                ui.set_editor_mode(0);
                ui.set_term_focused(false);
                ui.invoke_term_focus_changed(false);
                let is_asm = ui.get_is_active_asm();
                refresh_active_editor_highlight_and_labels(&ui, &ctrl, is_asm, true);
                ctrl.append_log(&format!("[DOCS] Inserted snippet: {}", snippet));
                sync_logs(&ui, &ctrl);
                sync_tabs_ui(&ui, &ctrl);
            }
            return;
        }

        let mut code = ui.get_code_text().to_string();
        if !code.is_empty() && !code.ends_with('\n') {
            code.push('\n');
        }
        code.push_str(&snippet);
        code.push('\n');

        let len = code.len() as i32;
        ui.set_code_text(SharedString::from(&code));
        ui.set_is_dirty(true);
        ui.set_editor_mode(0);
        ui.set_term_focused(false);
        ui.invoke_term_focus_changed(false);

        crate::ui::editor_bridge::record_editor_mutation(&ui, &mut ctrl, &code, len, false);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &code, is_asm);
        ctrl.append_log(&format!("[DOCS] Inserted snippet: {}", snippet));
        sync_logs(&ui, &ctrl);
    });
}
