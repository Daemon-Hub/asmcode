use crate::ui::AppWindow;
use crate::ui::controller::IdeController;
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
}
