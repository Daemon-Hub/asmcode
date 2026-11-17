use crate::session::AppSession;
use crate::ui::AppWindow;
use crate::ui::controller::IdeController;
use crate::ui::editor_bridge::{
    populate_docs_ui, refresh_suggestions_ui, update_highlight_and_labels,
};
use crate::ui::explorer_bridge::refresh_files_ui;
use crate::ui::handlers::{
    attach_autocomplete_handlers, attach_editor_handlers, attach_file_handlers,
    attach_toolchain_handlers, attach_view_handlers,
};
use crate::ui::terminal::update_terminal_lines;
use slint::SharedString;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub fn setup_ui(app: &AppWindow, controller_rc: Arc<Mutex<IdeController>>, session: AppSession) {
    // 0. Restore session settings
    app.set_editor_font_size(session.editor_font_size);
    app.set_editor_font_italic(session.editor_font_italic);
    let lang = if session.current_language.is_empty() {
        "ru".to_string()
    } else {
        session.current_language.clone()
    };
    app.set_current_language(SharedString::from(&lang));
    app.set_labels_expanded(session.labels_expanded);
    app.set_terminal_expanded(session.terminal_expanded);
    app.set_sidebar_width(session.sidebar_width);
    app.set_terminal_height(session.terminal_height);

    // Initial terminal lines
    {
        let ctrl = controller_rc.lock().unwrap();
        update_terminal_lines(app, &ctrl.logs);
    }

    // 1. Populate Docs List
    populate_docs_ui(app, &lang);

    // 2. Load File List
    refresh_files_ui(app, &controller_rc);

    // 3. Initial load of last opened file or 8080.asm/main.asm
    {
        let target_opt = if let Some(ref last_file) = session.last_open_file {
            let p = PathBuf::from(last_file);
            if p.exists() { Some(p) } else { None }
        } else {
            None
        }
        .or_else(|| {
            let ctrl = controller_rc.lock().unwrap();
            if ctrl.file_manager.current_dir.join("8080.asm").exists() {
                Some(ctrl.file_manager.current_dir.join("8080.asm"))
            } else if ctrl.file_manager.current_dir.join("main.asm").exists() {
                Some(ctrl.file_manager.current_dir.join("main.asm"))
            } else {
                None
            }
        });

        if let Some(target_file) = target_opt {
            let mut ctrl = controller_rc.lock().unwrap();
            if let Ok(content) = ctrl.file_manager.read_file_content(&target_file) {
                let fname = target_file
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let is_asm = fname.ends_with(".asm")
                    || fname.ends_with(".inc")
                    || fname.ends_with(".s")
                    || fname.ends_with(".mac");
                app.set_active_file_name(SharedString::from(&fname));
                app.set_active_file_path(SharedString::from(
                    target_file.to_string_lossy().to_string(),
                ));
                app.set_code_text(SharedString::from(&content));
                app.set_is_dirty(false);
                app.set_is_active_asm(is_asm);
                update_highlight_and_labels(app, &ctrl.autocomplete, &content, is_asm);
            }
        }
    }

    // 4. Initial suggestions
    refresh_suggestions_ui(app, &controller_rc, "", &lang);

    // 5. Connect all UI Callbacks across specialized domain modules
    attach_file_handlers(app, &controller_rc);
    attach_editor_handlers(app, &controller_rc);
    attach_autocomplete_handlers(app, &controller_rc);
    attach_toolchain_handlers(app, &controller_rc);
    attach_view_handlers(app, &controller_rc);
}
