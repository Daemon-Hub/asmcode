use crate::session::AppSession;
use crate::ui::AppWindow;
use crate::ui::controller::IdeController;
use crate::ui::editor_bridge::{populate_docs_ui, refresh_suggestions_ui};
use crate::ui::explorer_bridge::refresh_files_ui;
use crate::ui::handlers::{
    attach_autocomplete_handlers, attach_dnd_handlers, attach_editor_handlers,
    attach_file_handlers, attach_toolchain_handlers, attach_view_handlers, open_file_internal,
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
    app.set_active_theme_id(session.active_theme_id);
    app.set_cursor_blink(session.cursor_blink);
    app.set_tab_size(session.tab_size);
    if !session.compiler_path.is_empty() {
        app.set_compiler_path(SharedString::from(&session.compiler_path));
    }
    if !session.compiler_args.is_empty() {
        app.set_compiler_args(SharedString::from(&session.compiler_args));
    }
    if !session.emulator_path.is_empty() {
        app.set_emulator_path(SharedString::from(&session.emulator_path));
    }
    if !session.emulator_args.is_empty() {
        app.set_emulator_args(SharedString::from(&session.emulator_args));
    }
    app.set_toolchain_type(session.toolchain_type);
    app.set_associate_asm_files(crate::fs_manager::associations::is_asm_associated());

    {
        let mut ctrl = controller_rc.lock().unwrap();
        ctrl.session = session.clone();
        for tab_str in &session.open_tabs {
            let p = PathBuf::from(tab_str);
            if p.exists() && !ctrl.open_tabs.contains(&p) {
                ctrl.open_tabs.push(p);
            }
        }
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
            open_file_internal(app, &controller_rc, &target_file, false);
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
    attach_dnd_handlers(app, &controller_rc);
}
