use crate::compiler::runner::CompilerRunner;
use crate::ui::AppWindow;
use crate::ui::controller::IdeController;
use crate::ui::terminal::sync_logs;
use slint::{Color, ComponentHandle, SharedString};
use std::sync::{Arc, Mutex};

pub fn attach_toolchain_handlers(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
    // Assemble Only (asmx20)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_run_assemble_only(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();

        // 1. Check if file is open
        let active_name = ui.get_active_file_name().to_string();
        if active_name.is_empty() {
            ctrl.append_log("[WARN] No active file is currently open to assemble.");
            sync_logs(&ui, &ctrl);
            return;
        }

        // Save file first
        let text = ui.get_code_text().to_string();
        let target_path = if let Some(path) = &ctrl.file_manager.active_file_path {
            path.clone()
        } else {
            ctrl.file_manager.current_dir.join(&active_name)
        };

        if let Err(e) = ctrl.file_manager.write_file_content(&target_path, &text) {
            ctrl.append_log(&format!("[ERROR] Save before assemble failed: {}", e));
            sync_logs(&ui, &ctrl);
            return;
        }
        ui.set_is_dirty(false);
        ui.set_status_message(SharedString::from("Building (.COM)..."));
        ui.set_status_color(Color::from_rgb_u8(137, 180, 250));

        let toolchain_type = ui.get_toolchain_type();
        let compiler_path = ui.get_compiler_path().to_string();

        // 2. Assemble with selected toolchain on background thread (Never freezes UI)
        let app_weak_inner = app_weak.clone();
        let ctrl_clone_inner = Arc::clone(&ctrl_clone);
        std::thread::spawn(move || {
            let (report, warning) =
                CompilerRunner::assemble_file(&target_path, toolchain_type, &compiler_path);
            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = app_weak_inner.upgrade() else {
                    return;
                };
                let mut ctrl = ctrl_clone_inner.lock().unwrap();
                for log in &report.output_logs {
                    ctrl.append_log(log);
                }

                if report.success {
                    if warning.is_some() {
                        ui.set_status_message(SharedString::from(
                            "Предупреждение: бинарник не найден (применен встроенный)",
                        ));
                        ui.set_status_color(Color::from_rgb_u8(249, 226, 175));
                    } else {
                        ui.set_status_message(SharedString::from("Build OK (.COM generated)"));
                        ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
                    }
                } else {
                    ui.set_status_message(SharedString::from("Assembly Failed"));
                    ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
                }

                sync_logs(&ui, &ctrl);
            });
        });
    });

    // Run in Emulator (asmx20 + cpmemu or intel-8080-kit - Asynchronous, Never Freezes UI)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_run_emulator(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        if !ui.get_terminal_expanded() {
            ui.set_terminal_expanded(true);
        }
        let mut ctrl = ctrl_clone.lock().unwrap();

        // 1. Check if file is open
        let active_name = ui.get_active_file_name().to_string();
        if active_name.is_empty() {
            ctrl.append_log("[WARN] No active file is currently open to assemble.");
            sync_logs(&ui, &ctrl);
            return;
        }

        // Save file first
        let text = ui.get_code_text().to_string();
        let target_path = if let Some(path) = &ctrl.file_manager.active_file_path {
            path.clone()
        } else {
            ctrl.file_manager.current_dir.join(&active_name)
        };

        if let Err(e) = ctrl.file_manager.write_file_content(&target_path, &text) {
            ctrl.append_log(&format!("[ERROR] Save before run failed: {}", e));
            sync_logs(&ui, &ctrl);
            return;
        }
        ui.set_is_dirty(false);
        ui.set_status_message(SharedString::from("Assembling & Launching..."));
        ui.set_status_color(Color::from_rgb_u8(137, 180, 250));

        let toolchain_type = ui.get_toolchain_type();
        let compiler_path = ui.get_compiler_path().to_string();
        let emulator_path = ui.get_emulator_path().to_string();

        // 2. Assemble and Run on background thread (Never freezes UI)
        let app_weak_inner = app_weak.clone();
        let ctrl_clone_inner = Arc::clone(&ctrl_clone);
        std::thread::spawn(move || {
            let (report, compile_warn) =
                CompilerRunner::assemble_file(&target_path, toolchain_type, &compiler_path);
            let (emu_logs, emu_warn) = if report.success {
                if let Some(com_path) = report.com_path.as_deref() {
                    let (logs, warn) =
                        CompilerRunner::run_in_emulator(com_path, toolchain_type, &emulator_path);
                    (Some(logs), warn)
                } else {
                    (None, None)
                }
            } else {
                (None, None)
            };

            let _ = slint::invoke_from_event_loop(move || {
                let Some(ui) = app_weak_inner.upgrade() else {
                    return;
                };
                let mut ctrl = ctrl_clone_inner.lock().unwrap();
                for log in &report.output_logs {
                    ctrl.append_log(log);
                }

                if !report.success {
                    ui.set_status_message(SharedString::from("Build failed, run aborted"));
                    ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
                } else if let Some(logs) = emu_logs {
                    for log in logs {
                        ctrl.append_log(&log);
                    }
                    if compile_warn.is_some() || emu_warn.is_some() {
                        ui.set_status_message(SharedString::from(
                            "Выполнено (с предупреждением: бинарник не найден, применен встроенный)",
                        ));
                        ui.set_status_color(Color::from_rgb_u8(249, 226, 175));
                    } else if toolchain_type == 1 {
                        ui.set_status_message(SharedString::from("Execution Complete (cpmemu)"));
                        ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
                    } else {
                        ui.set_status_message(SharedString::from(
                            "Execution Complete (intel-8080-kit)",
                        ));
                        ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
                    }
                }

                sync_logs(&ui, &ctrl);
            });
        });
    });

    // Run Syntax Check
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_run_syntax_check(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        let code = ui.get_code_text().to_string();
        let report = CompilerRunner::validate_syntax(&code);

        for log in &report.output_logs {
            ctrl.append_log(log);
        }

        if report.success {
            ui.set_status_message(SharedString::from("Syntax Valid"));
            ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
        } else {
            ui.set_status_message(SharedString::from("Syntax Errors Found"));
            ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
        }

        sync_logs(&ui, &ctrl);
    });

    // Clear Output Console
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_clear_output(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        ctrl.logs.clear();
        ctrl.logs.push("[CONSOLE] Output logs cleared.".to_string());
        sync_logs(&ui, &ctrl);
    });
}
