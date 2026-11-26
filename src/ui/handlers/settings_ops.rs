use crate::ui::AppWindow;
use crate::ui::controller::{IdeController, persist_current_session};
use crate::ui::terminal::sync_logs;
use slint::{Color, ComponentHandle, SharedString, Weak};
use std::sync::{Arc, Mutex};

fn sync_and_persist_settings(ui: &AppWindow, ctrl: &mut IdeController) {
    ctrl.session.large_file_optimizations = ui.get_large_file_optimizations();
    ctrl.session.large_file_soft_limit_mb = ui.get_large_file_soft_limit_mb().max(1) as u64;
    ctrl.session.large_file_hard_limit_mb = ui.get_large_file_hard_limit_mb().max(1) as u64;
    persist_current_session(ui, ctrl);
}

fn pick_executable_async(
    app_weak: Weak<AppWindow>,
    ctrl_rc: Arc<Mutex<IdeController>>,
    title: &'static str,
    log_label: &'static str,
    apply_path: fn(&AppWindow, SharedString),
) {
    std::thread::spawn(move || {
        let mut dialog = rfd::FileDialog::new().set_title(title);
        #[cfg(windows)]
        {
            dialog = dialog.add_filter(
                "Executable (*.exe, *.com, *.bat)",
                &["exe", "com", "bat", "cmd"],
            );
        }
        dialog = dialog.add_filter("All Files (*.*)", &["*"]);

        if let Some(path) = dialog.pick_file() {
            let path_str = path.to_string_lossy().to_string();
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = app_weak.upgrade() else {
                    return;
                };
                apply_path(&ui, SharedString::from(&path_str));
                let mut ctrl = ctrl_rc.lock().unwrap();
                ctrl.append_log(&format!("[TOOLCHAIN] Selected {}: {}", log_label, path_str));
                sync_logs(&ui, &ctrl);
                sync_and_persist_settings(&ui, &mut ctrl);
            });
        }
    });
}

pub fn attach_settings_handlers(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
    // Save Settings
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_save_settings(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let lang = ui.get_current_language().to_string();
        let is_asm = ui.get_is_active_asm();
        {
            let mut ctrl = ctrl_clone.lock().unwrap();
            ctrl.append_log("[SETTINGS] Configuration updated and persisted");
            ctrl.invalidate_viewport_cache();
            crate::ui::editor_bridge::reset_editor_ui_caches();
            crate::ui::editor_bridge::refresh_active_editor_highlight_and_labels(
                &ui, &ctrl, is_asm, true,
            );
            crate::ui::editor_bridge::populate_docs_ui(&ui, &lang);
            let msg = if lang == "ru" {
                "Настройки сохранены"
            } else {
                "Settings saved"
            };
            ui.set_status_message(SharedString::from(msg));
            ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
            sync_logs(&ui, &ctrl);
            sync_and_persist_settings(&ui, &mut ctrl);
        }
        crate::ui::editor_bridge::refresh_suggestions_ui(
            &ui,
            &ctrl_clone,
            ui.get_search_filter().as_str(),
            &lang,
        );
    });

    // Reset Settings Defaults
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_reset_settings_defaults(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        ui.set_editor_font_size(18.0);
        ui.set_editor_font_italic(false);
        ui.set_cursor_blink(true);
        ui.set_tab_size(4);
        ui.set_active_theme_id(1);
        ui.set_toolchain_type(0);
        ui.set_large_file_optimizations(true);
        ui.set_large_file_soft_limit_mb(20);
        ui.set_large_file_hard_limit_mb(500);
        ui.set_compiler_args(SharedString::from("-C 8080 -e -w -b 0x100 -o \"{output}\" \"{input}\""));
        ui.set_emulator_args(SharedString::from("--8080 \"{output}\""));

        let lang = ui.get_current_language().to_string();
        let is_asm = ui.get_is_active_asm();
        {
            let mut ctrl = ctrl_clone.lock().unwrap();
            ctrl.invalidate_viewport_cache();
            crate::ui::editor_bridge::reset_editor_ui_caches();
            crate::ui::editor_bridge::refresh_active_editor_highlight_and_labels(
                &ui, &ctrl, is_asm, true,
            );
            crate::ui::editor_bridge::populate_docs_ui(&ui, &lang);
            ctrl.append_log(
                "[SETTINGS] Defaults restored (18px, Cyberpunk 2077 Red, built-in toolchain)",
            );
            sync_logs(&ui, &ctrl);
            sync_and_persist_settings(&ui, &mut ctrl);
        }
        crate::ui::editor_bridge::refresh_suggestions_ui(
            &ui,
            &ctrl_clone,
            ui.get_search_filter().as_str(),
            &lang,
        );
    });

    // Browse Compiler Executable
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_browse_compiler(move || {
        pick_executable_async(
            app_weak.clone(),
            Arc::clone(&ctrl_clone),
            "Select Intel 8080 Compiler Executable",
            "compiler",
            |ui, s| ui.set_compiler_path(s),
        );
    });

    // Browse Emulator Executable
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_browse_emulator(move || {
        pick_executable_async(
            app_weak.clone(),
            Arc::clone(&ctrl_clone),
            "Select CP/M Emulator Executable",
            "emulator",
            |ui, s| ui.set_emulator_path(s),
        );
    });

    // Toggle File Association for .asm
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_toggle_associate_asm(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let new_state = ui.get_associate_asm_files();
        match crate::fs_manager::associations::set_asm_association(new_state) {
            Ok(()) => {
                let mut ctrl = ctrl_clone.lock().unwrap();
                ctrl.session.associate_asm_files = new_state;
                let msg = if new_state {
                    "[SYSTEM] Registered .asm file association with ASM Code in OS"
                } else {
                    "[SYSTEM] Removed .asm file association in OS"
                };
                ctrl.append_log(msg);
                sync_logs(&ui, &ctrl);
                sync_and_persist_settings(&ui, &mut ctrl);
            }
            Err(e) => {
                ui.set_associate_asm_files(!new_state);
                let mut ctrl = ctrl_clone.lock().unwrap();
                ctrl.append_log(&format!("[ERROR] File association failed: {}", e));
                sync_logs(&ui, &ctrl);
            }
        }
    });
}
