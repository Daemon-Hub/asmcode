use crate::compiler::runner::{get_timestamp, CompilerRunner};
use crate::ui::AppWindow;
use crate::ui::controller::{IdeController, RunningProcess};
use crate::ui::terminal::sync_logs;
use slint::{Color, ComponentHandle, SharedString};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

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
                        ui.set_status_message(SharedString::from("Собрано (с предупреждением)"));
                        ui.set_status_color(Color::from_rgb_u8(249, 226, 175));
                    } else {
                        ui.set_status_message(SharedString::from("Build Succeeded (.COM created)"));
                        ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
                    }
                } else {
                    ui.set_status_message(SharedString::from("Build Failed"));
                    ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
                }

                sync_logs(&ui, &ctrl);
            });
        });
    });

    // Run in Emulator with Live Interactive Stdin/Stdout
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

        // 2. Assemble and Run on background thread
        let app_weak_inner = app_weak.clone();
        let ctrl_clone_inner = Arc::clone(&ctrl_clone);
        std::thread::spawn(move || {
            let (report, compile_warn) =
                CompilerRunner::assemble_file(&target_path, toolchain_type, &compiler_path);

            let app_w_report = app_weak_inner.clone();
            let ctrl_c_report = Arc::clone(&ctrl_clone_inner);
            let report_logs = report.output_logs.clone();
            let is_success = report.success;

            let _ = slint::invoke_from_event_loop(move || {
                if let Some(ui) = app_w_report.upgrade() {
                    let mut ctrl = ctrl_c_report.lock().unwrap();
                    for log in &report_logs {
                        ctrl.append_log(log);
                    }
                    if !is_success {
                        ui.set_status_message(SharedString::from("Build failed, run aborted"));
                        ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
                    }
                    sync_logs(&ui, &ctrl);
                }
            });

            if !is_success {
                return;
            }

            let Some(com_path) = report.com_path else {
                return;
            };

            // Launch Emulator (Interactive streaming for cpmemu, or builtin for intel-8080-kit)
            let trimmed = emulator_path.trim().to_string();
            let cpmemu_path = PathBuf::from(&trimmed);
            let use_external = toolchain_type == 1 && !trimmed.is_empty() && cpmemu_path.exists();

            if use_external {
                let parent_dir = com_path.parent().unwrap_or_else(|| Path::new("."));
                let mut cmd = std::process::Command::new(&cpmemu_path);
                #[cfg(windows)]
                {
                    use std::os::windows::process::CommandExt;
                    cmd.creation_flags(CREATE_NO_WINDOW);
                }
                cmd.current_dir(parent_dir)
                    .args(["--8080", &com_path.to_string_lossy()])
                    .stdin(std::process::Stdio::piped())
                    .stdout(std::process::Stdio::piped())
                    .stderr(std::process::Stdio::piped());

                let app_w_start = app_weak_inner.clone();
                let ctrl_c_start = Arc::clone(&ctrl_clone_inner);
                let cpmemu_display = cpmemu_path.display().to_string();
                let com_display = com_path.display().to_string();

                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = app_w_start.upgrade() {
                        let mut ctrl = ctrl_c_start.lock().unwrap();
                        ctrl.append_log(&format!(
                            "[{}] [EMULATOR:cpmemu] Running interactive CP/M 8080: {}",
                            get_timestamp(),
                            com_display
                        ));
                        ctrl.append_log(&format!(
                            "[{}] [CMD] {} --8080 {}",
                            get_timestamp(),
                            cpmemu_display,
                            com_display
                        ));
                        sync_logs(&ui, &ctrl);
                    }
                });

                match cmd.spawn() {
                    Ok(mut child) => {
                        let pid = child.id();
                        let child_stdin = child.stdin.take().expect("Failed to open child stdin");
                        let mut child_stdout = child.stdout.take().expect("Failed to open child stdout");
                        let mut child_stderr = child.stderr.take().expect("Failed to open child stderr");
                        let stdin_arc = Arc::new(Mutex::new(child_stdin));

                        // Store active process in controller
                        {
                            let mut ctrl = ctrl_clone_inner.lock().unwrap();
                            ctrl.running_process = Some(RunningProcess {
                                pid,
                                stdin: Arc::clone(&stdin_arc),
                            });
                        }

                        let app_w_ui = app_weak_inner.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = app_w_ui.upgrade() {
                                ui.set_is_process_running(true);
                                ui.set_status_message(SharedString::from("Running (cpmemu)..."));
                                ui.set_status_color(Color::from_rgb_u8(137, 220, 235));
                            }
                        });

                        // Stdout background reader thread: real-time chunk streaming (never blocks waiting for newline)
                        let app_w_out = app_weak_inner.clone();
                        let ctrl_c_out = Arc::clone(&ctrl_clone_inner);
                        std::thread::spawn(move || {
                            use std::io::Read;
                            let mut buf = [0u8; 1024];
                            while let Ok(n) = child_stdout.read(&mut buf) {
                                if n == 0 {
                                    break;
                                }
                                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                                let w = app_w_out.clone();
                                let c = Arc::clone(&ctrl_c_out);
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = w.upgrade() {
                                        let mut ctrl = c.lock().unwrap();
                                        for line in text.split_inclusive('\n') {
                                            let cleaned = line.trim_end_matches(['\r', '\n']);
                                            if !cleaned.is_empty() {
                                                ctrl.append_log(&format!("[CPM] {}", cleaned));
                                            }
                                        }
                                        sync_logs(&ui, &ctrl);
                                    }
                                });
                            }
                        });

                        // Stderr background reader thread: real-time chunk streaming
                        let app_w_err = app_weak_inner.clone();
                        let ctrl_c_err = Arc::clone(&ctrl_clone_inner);
                        std::thread::spawn(move || {
                            use std::io::Read;
                            let mut buf = [0u8; 1024];
                            while let Ok(n) = child_stderr.read(&mut buf) {
                                if n == 0 {
                                    break;
                                }
                                let text = String::from_utf8_lossy(&buf[..n]).to_string();
                                let w = app_w_err.clone();
                                let c = Arc::clone(&ctrl_c_err);
                                let _ = slint::invoke_from_event_loop(move || {
                                    if let Some(ui) = w.upgrade() {
                                        let mut ctrl = c.lock().unwrap();
                                        for line in text.split_inclusive('\n') {
                                            let cleaned = line.trim_end_matches(['\r', '\n']);
                                            if !cleaned.is_empty() {
                                                ctrl.append_log(&format!("[CPM ERR] {}", cleaned));
                                            }
                                        }
                                        sync_logs(&ui, &ctrl);
                                    }
                                });
                            }
                        });

                        // Wait for process completion without locks
                        let app_w_wait = app_weak_inner.clone();
                        let ctrl_c_wait = Arc::clone(&ctrl_clone_inner);
                        std::thread::spawn(move || {
                            let status = child.wait();
                            let _ = slint::invoke_from_event_loop(move || {
                                if let Some(ui) = app_w_wait.upgrade() {
                                    let mut ctrl = ctrl_c_wait.lock().unwrap();
                                    ctrl.running_process = None;
                                    ui.set_is_process_running(false);
                                    match status {
                                        Ok(s) => {
                                            ctrl.append_log(&format!(
                                                "[{}] [EMULATOR:cpmemu] Process finished with exit code {:?}",
                                                get_timestamp(),
                                                s.code()
                                            ));
                                            ui.set_status_message(SharedString::from("Execution Complete (cpmemu)"));
                                            ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
                                        }
                                        Err(e) => {
                                            ctrl.append_log(&format!("[ERROR] Process wait failed: {}", e));
                                            ui.set_status_message(SharedString::from("Process Terminated"));
                                            ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
                                        }
                                    }
                                    sync_logs(&ui, &ctrl);
                                }
                            });
                        });
                    }
                    Err(err) => {
                        let app_w_err = app_weak_inner.clone();
                        let ctrl_c_err = Arc::clone(&ctrl_clone_inner);
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = app_w_err.upgrade() {
                                let mut ctrl = ctrl_c_err.lock().unwrap();
                                ctrl.append_log(&format!(
                                    "[{}] [ERROR] Failed to run external emulator: {}",
                                    get_timestamp(),
                                    err
                                ));
                                ui.set_status_message(SharedString::from("Emulator Launch Failed"));
                                ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
                                sync_logs(&ui, &ctrl);
                            }
                        });
                    }
                }
            } else {
                // Built-in Pure-Rust Emulator (intel-8080-kit)
                let warn = if toolchain_type == 1 {
                    if trimmed.is_empty() {
                        Some("[ПРЕДУПРЕЖДЕНИЕ] Путь к внешнему эмулятору не указан. Применен встроенный эмулятор по умолчанию (intel-8080-kit).".to_string())
                    } else {
                        Some(format!(
                            "[ПРЕДУПРЕЖДЕНИЕ] Внешний эмулятор не найден по пути '{}'. Применен встроенный эмулятор по умолчанию (intel-8080-kit).",
                            trimmed
                        ))
                    }
                } else {
                    None
                };

                let mut logs = CompilerRunner::run_emulator_builtin(&com_path);
                if let Some(w) = warn {
                    logs.insert(0, w);
                }

                let app_w_builtin = app_weak_inner.clone();
                let ctrl_c_builtin = Arc::clone(&ctrl_clone_inner);
                let _ = slint::invoke_from_event_loop(move || {
                    if let Some(ui) = app_w_builtin.upgrade() {
                        let mut ctrl = ctrl_c_builtin.lock().unwrap();
                        for log in logs {
                            ctrl.append_log(&log);
                        }
                        if compile_warn.is_some() {
                            ui.set_status_message(SharedString::from(
                                "Выполнено (с предупреждением: бинарник не найден)",
                            ));
                            ui.set_status_color(Color::from_rgb_u8(249, 226, 175));
                        } else {
                            ui.set_status_message(SharedString::from(
                                "Execution Complete (intel-8080-kit)",
                            ));
                            ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
                        }
                        sync_logs(&ui, &ctrl);
                    }
                });
            }
        });
    });

    // Terminal Stdin Input Submitted by User
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_terminal_input_submitted(move |input_text| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        let text = input_text.to_string();

        if let Some(ref proc) = ctrl.running_process {
            use std::io::Write;
            if let Ok(mut stdin) = proc.stdin.lock() {
                let payload = format!("{}\r\n", text);
                let _ = stdin.write_all(payload.as_bytes());
                let _ = stdin.flush();
            }
        }
        ctrl.append_log(&format!("[STDIN] {}", text));
        sync_logs(&ui, &ctrl);
    });

    // Stop / Terminate Running Process by PID (Non-blocking, never freezes UI)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_stop_process(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        if let Some(proc) = ctrl.running_process.take() {
            let pid = proc.pid;
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let mut kill_cmd = std::process::Command::new("taskkill");
                kill_cmd.args(["/F", "/T", "/PID", &pid.to_string()]);
                kill_cmd.creation_flags(CREATE_NO_WINDOW);
                let _ = kill_cmd.spawn();
            }
            #[cfg(not(windows))]
            {
                let _ = std::process::Command::new("kill").args(["-9", &pid.to_string()]).spawn();
            }
            ctrl.append_log("[EMULATOR] Process terminated by user.");
            ui.set_is_process_running(false);
            ui.set_status_message(SharedString::from("Stopped"));
            ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
            sync_logs(&ui, &ctrl);
        }
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
