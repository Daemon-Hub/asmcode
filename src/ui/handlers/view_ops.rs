use crate::ui::AppWindow;
use crate::ui::controller::{IdeController, persist_current_session};
use crate::ui::editor_bridge::update_highlight_and_labels;
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
    });

    // Switch Editor Theme (0 = Catppuccin Dark, 1 = Cyberpunk 2077 Red)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_switch_theme(move |new_theme_id| {
        let Some(ui) = app_weak.upgrade() else { return };
        ui.set_active_theme_id(new_theme_id);

        let code = ui.get_code_text().to_string();
        let is_asm = ui.get_is_active_asm();
        {
            let mut ctrl = ctrl_clone.lock().unwrap();
            let theme_name = if new_theme_id == 1 {
                "Cyberpunk 2077 Red"
            } else {
                "Catppuccin Dark"
            };
            ctrl.append_log(&format!("[THEME] Active theme: {}", theme_name));
            update_highlight_and_labels(&ui, &ctrl.autocomplete, &code, is_asm);
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
        crate::ui::editor_bridge::populate_filtered_docs_ui(&ui, &lang, &category);
    });

    // Insert Doc Snippet into Active Editor Code
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_insert_doc_snippet(move |snippet| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut code = ui.get_code_text().to_string();
        if !code.is_empty() && !code.ends_with('\n') {
            code.push('\n');
        }
        code.push_str(&snippet);
        code.push('\n');

        let len = code.len() as i32;
        ui.set_code_text(SharedString::from(&code));
        ui.set_is_dirty(true);
        ui.set_editor_mode(0); // Switch view back to Editor

        let mut ctrl = ctrl_clone.lock().unwrap();
        crate::ui::editor_bridge::record_editor_mutation(&ui, &mut ctrl, &code, len, false);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &code, is_asm);
        ctrl.append_log(&format!("[DOCS] Inserted snippet: {}", snippet));
        sync_logs(&ui, &ctrl);
    });
}
