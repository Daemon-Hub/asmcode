use crate::compiler::runner::CompilerRunner;
use crate::ui::AppWindow;
use crate::ui::controller::IdeController;
use crate::ui::terminal::sync_logs;
use slint::{Color, ComponentHandle, SharedString};
use std::path::Path;
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

        // 3. Expand and switch bottom panel to Terminal tab
        ui.set_terminal_expanded(true);
        ui.set_terminal_active_tab(1);
        ui.set_term_focused(true);
        ui.invoke_term_focus_changed(true);

        // 4. Build command line based on toolchain and shell
        let toolchain_type = ui.get_toolchain_type();
        let compiler_path = ui.get_compiler_path().to_string();
        let compiler_args = ui.get_compiler_args().to_string();
        let emulator_path = ui.get_emulator_path().to_string();
        let emulator_args = ui.get_emulator_args().to_string();
        let shell_name = ui.get_term_shell_name().to_string();

        if toolchain_type == 0 {
            // Built-in toolchain: assemble in memory first
            ui.set_status_message(SharedString::from("Компиляция (.COM)..."));
            ui.set_status_color(Color::from_rgb_u8(137, 180, 250));
            let report = CompilerRunner::assemble_file_builtin(&target_path);
            for log in &report.output_logs {
                ctrl.append_log(log);
            }
            sync_logs(&ui, &ctrl);

            if !report.success {
                ui.set_status_message(SharedString::from("Ошибка сборки"));
                ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
                return;
            }

            let cmd = build_emulator_command(
                &shell_name,
                &emulator_path,
                &emulator_args,
                &target_path,
            );
            ctrl.append_log(&format!("[RUN] Terminal: {}", cmd));
            sync_logs(&ui, &ctrl);
            ui.invoke_term_send_command(SharedString::from(cmd));
            ui.set_status_message(SharedString::from("Запущено в терминале"));
            ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
        } else {
            // External toolchain: send combined compile + run command to terminal
            let cmd = build_combined_run_command(
                &shell_name,
                &compiler_path,
                &compiler_args,
                &emulator_path,
                &emulator_args,
                &target_path,
            );
            ctrl.append_log(&format!("[RUN] Terminal: {}", cmd));
            sync_logs(&ui, &ctrl);
            ui.invoke_term_send_command(SharedString::from(cmd));
            ui.set_status_message(SharedString::from("Запущено в терминале"));
            ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
        }
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

    // Terminal Expanded State Changed (Persist session immediately)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_terminal_expanded_changed(move |expanded| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        ctrl.session.terminal_expanded = expanded;
        crate::ui::controller::persist_current_session(&ui, &ctrl);
    });
}

pub fn build_combined_run_command(
    shell: &str,
    compiler_path: &str,
    compiler_args: &str,
    emulator_path: &str,
    emulator_args: &str,
    target_path: &Path,
) -> String {
    let com_path = target_path.with_extension("com");
    let input_str = target_path.to_string_lossy();
    let output_str = com_path.to_string_lossy();
    let stem_str = target_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let dir_str = target_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".into());

    let c_bin = if compiler_path.trim().is_empty() {
        #[cfg(windows)]
        {
            "C:\\bin\\asmx20.exe"
        }
        #[cfg(not(windows))]
        {
            "/usr/local/bin/asmx20"
        }
    } else {
        compiler_path.trim()
    };

    let e_bin = if emulator_path.trim().is_empty() {
        #[cfg(windows)]
        {
            "C:\\bin\\cpmemu.exe"
        }
        #[cfg(not(windows))]
        {
            "/usr/local/bin/cpmemu"
        }
    } else {
        emulator_path.trim()
    };

    let c_args_tpl = if compiler_args.trim().is_empty() {
        "-C 8080 -e -w -b 0x100 -o \"{output}\" \"{input}\""
    } else {
        compiler_args.trim()
    };

    let e_args_tpl = if emulator_args.trim().is_empty() {
        "--8080 \"{output}\""
    } else {
        emulator_args.trim()
    };

    let mut formatted_c_args = c_args_tpl.to_string();
    if formatted_c_args.contains("{input}") || formatted_c_args.contains("{file}") {
        formatted_c_args = formatted_c_args
            .replace("{input}", &input_str)
            .replace("{file}", &input_str);
    } else {
        formatted_c_args.push_str(&format!(" \"{}\"", input_str));
    }
    formatted_c_args = formatted_c_args
        .replace("{output}", &output_str)
        .replace("{bin}", &output_str)
        .replace("{com}", &output_str)
        .replace("{name}", &stem_str)
        .replace("{dir}", &dir_str);

    let mut formatted_e_args = e_args_tpl.to_string();
    if formatted_e_args.contains("{output}")
        || formatted_e_args.contains("{bin}")
        || formatted_e_args.contains("{com}")
    {
        formatted_e_args = formatted_e_args
            .replace("{output}", &output_str)
            .replace("{bin}", &output_str)
            .replace("{com}", &output_str);
    } else {
        formatted_e_args.push_str(&format!(" \"{}\"", output_str));
    }
    formatted_e_args = formatted_e_args
        .replace("{name}", &stem_str)
        .replace("{dir}", &dir_str);

    let is_powershell = shell.eq_ignore_ascii_case("pwsh")
        || shell.eq_ignore_ascii_case("powershell")
        || (!shell.eq_ignore_ascii_case("cmd") && !shell.eq_ignore_ascii_case("sh") && cfg!(windows));

    if is_powershell {
        format!(
            "& \"{}\" {}; if ($?) {{ & \"{}\" {} }}",
            c_bin, formatted_c_args, e_bin, formatted_e_args
        )
    } else {
        format!(
            "\"{}\" {} && \"{}\" {}",
            c_bin, formatted_c_args, e_bin, formatted_e_args
        )
    }
}

pub fn build_emulator_command(
    shell: &str,
    emulator_path: &str,
    emulator_args: &str,
    target_path: &Path,
) -> String {
    let com_path = target_path.with_extension("com");
    let output_str = com_path.to_string_lossy();
    let stem_str = target_path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_default();
    let dir_str = target_path
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|| ".".into());

    let e_bin = if emulator_path.trim().is_empty() {
        #[cfg(windows)]
        {
            "C:\\bin\\cpmemu.exe"
        }
        #[cfg(not(windows))]
        {
            "/usr/local/bin/cpmemu"
        }
    } else {
        emulator_path.trim()
    };

    let e_args_tpl = if emulator_args.trim().is_empty() {
        "--8080 \"{output}\""
    } else {
        emulator_args.trim()
    };

    let mut formatted_e_args = e_args_tpl.to_string();
    if formatted_e_args.contains("{output}")
        || formatted_e_args.contains("{bin}")
        || formatted_e_args.contains("{com}")
    {
        formatted_e_args = formatted_e_args
            .replace("{output}", &output_str)
            .replace("{bin}", &output_str)
            .replace("{com}", &output_str);
    } else {
        formatted_e_args.push_str(&format!(" \"{}\"", output_str));
    }
    formatted_e_args = formatted_e_args
        .replace("{name}", &stem_str)
        .replace("{dir}", &dir_str);

    let is_powershell = shell.eq_ignore_ascii_case("pwsh")
        || shell.eq_ignore_ascii_case("powershell")
        || (!shell.eq_ignore_ascii_case("cmd") && !shell.eq_ignore_ascii_case("sh") && cfg!(windows));

    if is_powershell {
        format!("& \"{}\" {}", e_bin, formatted_e_args)
    } else {
        format!("\"{}\" {}", e_bin, formatted_e_args)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_combined_run_command_powershell() {
        let target = Path::new("C:\\bin\\8080.test.asm");
        let cmd = build_combined_run_command(
            "pwsh",
            "C:\\bin\\asmx20.exe",
            "-C 8080 -e -w -b 0x100 -o \"{output}\" \"{input}\"",
            "C:\\bin\\cpmemu.exe",
            "--8080 \"{output}\"",
            target,
        );
        assert!(cmd.starts_with("& \"C:\\bin\\asmx20.exe\" -C 8080"));
        assert!(cmd.contains("if ($?) { & \"C:\\bin\\cpmemu.exe\" --8080"));
    }

    #[test]
    fn test_build_combined_run_command_cmd() {
        let target = Path::new("C:\\bin\\8080.test.asm");
        let cmd = build_combined_run_command(
            "cmd",
            "C:\\bin\\asmx20.exe",
            "-o \"{output}\" \"{input}\"",
            "C:\\bin\\cpmemu.exe",
            "\"{output}\"",
            target,
        );
        assert!(cmd.starts_with("\"C:\\bin\\asmx20.exe\" -o "));
        assert!(cmd.contains(" && \"C:\\bin\\cpmemu.exe\""));
    }
}
