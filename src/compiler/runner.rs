use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

#[derive(Clone, Debug)]
pub struct Diagnostic {
    pub line: usize,
    pub message: String,
    pub is_error: bool,
}

#[derive(Clone, Debug)]
#[allow(dead_code)]
pub struct CompileReport {
    pub success: bool,
    pub output_logs: Vec<String>,
    pub diagnostics: Vec<Diagnostic>,
    pub instruction_count: usize,
    pub estimated_bytes: usize,
    pub com_path: Option<PathBuf>,
}

pub struct CompilerRunner;

impl CompilerRunner {
    pub fn assemble_file_builtin(file_path: &Path) -> CompileReport {
        crate::compiler::kit_engine::assemble_file(file_path)
    }

    pub fn assemble_file_external(file_path: &Path, asmx_bin: &Path) -> CompileReport {
        let mut logs = Vec::new();
        let com_path = file_path.with_extension("com");

        logs.push(format!(
            "[{}] [ASSEMBLER:asmx] Starting assembly for: {}",
            get_timestamp(),
            file_path.display()
        ));
        logs.push(format!(
            "[{}] [CMD] {} -C 8080 -e -w -b 0x100 -o {} {}",
            get_timestamp(),
            asmx_bin.display(),
            com_path.display(),
            file_path.display()
        ));

        let parent_dir = file_path.parent().unwrap_or_else(|| Path::new("."));

        let mut cmd = Command::new(asmx_bin);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let output = match cmd
            .current_dir(parent_dir)
            .args([
                "-C",
                "8080",
                "-e",
                "-w",
                "-b",
                "0x100",
                "-o",
                &com_path.to_string_lossy(),
                &file_path.to_string_lossy(),
            ])
            .output()
        {
            Ok(out) => out,
            Err(err) => {
                logs.push(format!(
                    "[{}] [ERROR] Failed to execute external assembler ({}): {}",
                    get_timestamp(),
                    asmx_bin.display(),
                    err
                ));
                return CompileReport {
                    success: false,
                    output_logs: logs,
                    diagnostics: vec![Diagnostic {
                        line: 1,
                        message: format!("Failed to run external assembler: {}", err),
                        is_error: true,
                    }],
                    instruction_count: 0,
                    estimated_bytes: 0,
                    com_path: None,
                };
            }
        };

        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);

        if !stdout.is_empty() {
            for line in stdout.lines() {
                logs.push(format!("[asmx20] {}", line));
            }
        }
        if !stderr.is_empty() {
            for line in stderr.lines() {
                logs.push(format!("[asmx20:err] {}", line));
            }
        }

        let success = output.status.success() && com_path.exists();
        if success {
            let file_size = std::fs::metadata(&com_path).map(|m| m.len()).unwrap_or(0);
            logs.push(format!(
                "[{}] [SUCCESS:asmx] Generated binary: {} ({} bytes)",
                get_timestamp(),
                com_path.file_name().unwrap_or_default().to_string_lossy(),
                file_size
            ));
        } else {
            logs.push(format!(
                "[{}] [ERROR:asmx] Assembly failed with exit code: {:?}",
                get_timestamp(),
                output.status.code()
            ));
        }

        CompileReport {
            success,
            output_logs: logs,
            diagnostics: Vec::new(),
            instruction_count: 0,
            estimated_bytes: 0,
            com_path: if success { Some(com_path) } else { None },
        }
    }

    pub fn assemble_file(
        file_path: &Path,
        toolchain_type: i32,
        custom_asmx: &str,
    ) -> (CompileReport, Option<String>) {
        if toolchain_type == 1 {
            let trimmed = custom_asmx.trim();
            let asmx_path = PathBuf::from(trimmed);
            if !trimmed.is_empty() && asmx_path.exists() {
                let report = Self::assemble_file_external(file_path, &asmx_path);
                (report, None)
            } else {
                let warn = if trimmed.is_empty() {
                    "[ПРЕДУПРЕЖДЕНИЕ] Путь к внешнему компилятору не указан. Применен встроенный тулчейн по умолчанию (intel-8080-kit).".to_string()
                } else {
                    format!(
                        "[ПРЕДУПРЕЖДЕНИЕ] Внешний компилятор не найден по пути '{}'. Применен встроенный тулчейн по умолчанию (intel-8080-kit).",
                        trimmed
                    )
                };
                let mut report = Self::assemble_file_builtin(file_path);
                report.output_logs.insert(0, warn.clone());
                (report, Some(warn))
            }
        } else {
            let report = Self::assemble_file_builtin(file_path);
            (report, None)
        }
    }

    pub fn run_emulator_builtin(com_path: &Path) -> Vec<String> {
        crate::compiler::kit_engine::run_com_file(com_path)
    }

    pub fn run_emulator_external(com_path: &Path, cpmemu_bin: &Path) -> Vec<String> {
        let mut logs = Vec::new();

        logs.push(format!(
            "[{}] [EMULATOR:cpmemu] Running in CP/M 8080 Emulator: {}",
            get_timestamp(),
            com_path.display()
        ));
        logs.push(format!(
            "[{}] [CMD] {} --8080 {}",
            get_timestamp(),
            cpmemu_bin.display(),
            com_path.display()
        ));

        let parent_dir = com_path.parent().unwrap_or_else(|| Path::new("."));

        let mut cmd = Command::new(cpmemu_bin);
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        match cmd
            .current_dir(parent_dir)
            .args(["--8080", &com_path.to_string_lossy()])
            .output()
        {
            Ok(output) => {
                let stdout = String::from_utf8_lossy(&output.stdout);
                let stderr = String::from_utf8_lossy(&output.stderr);

                if !stdout.is_empty() {
                    for line in stdout.lines() {
                        logs.push(format!("[CPM OUTPUT] {}", line));
                    }
                }
                if !stderr.is_empty() {
                    for line in stderr.lines() {
                        logs.push(format!("[CPM ERR] {}", line));
                    }
                }
                logs.push(format!(
                    "[{}] [EMULATOR:cpmemu] Process finished with exit code {:?}",
                    get_timestamp(),
                    output.status.code()
                ));
            }
            Err(err) => {
                logs.push(format!(
                    "[{}] [ERROR] Failed to run external emulator ({}): {}",
                    get_timestamp(),
                    cpmemu_bin.display(),
                    err
                ));
            }
        }

        logs
    }

    pub fn run_in_emulator(
        com_path: &Path,
        toolchain_type: i32,
        custom_cpmemu: &str,
    ) -> (Vec<String>, Option<String>) {
        if toolchain_type == 1 {
            let trimmed = custom_cpmemu.trim();
            let cpmemu_path = PathBuf::from(trimmed);
            if !trimmed.is_empty() && cpmemu_path.exists() {
                let logs = Self::run_emulator_external(com_path, &cpmemu_path);
                (logs, None)
            } else {
                let warn = if trimmed.is_empty() {
                    "[ПРЕДУПРЕЖДЕНИЕ] Путь к внешнему эмулятору не указан. Применен встроенный эмулятор по умолчанию (intel-8080-kit).".to_string()
                } else {
                    format!(
                        "[ПРЕДУПРЕЖДЕНИЕ] Внешний эмулятор не найден по пути '{}'. Применен встроенный эмулятор по умолчанию (intel-8080-kit).",
                        trimmed
                    )
                };
                let mut logs = Self::run_emulator_builtin(com_path);
                logs.insert(0, warn.clone());
                (logs, Some(warn))
            }
        } else {
            let logs = Self::run_emulator_builtin(com_path);
            (logs, None)
        }
    }

    pub fn validate_syntax(source_code: &str) -> CompileReport {
        crate::syntax::validator::validate_syntax(source_code)
    }
}

fn get_timestamp() -> String {
    use std::time::SystemTime;
    let duration = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default();
    let total_secs = duration.as_secs();
    let seconds = total_secs % 60;
    let minutes = (total_secs / 60) % 60;
    let hours = (total_secs / 3600) % 24;
    format!("{:02}:{:02}:{:02}", hours, minutes, seconds)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_assemble_file_default_builtin() {
        let temp_dir = std::env::temp_dir();
        let asm_path = temp_dir.join("test_default_toolchain.asm");
        std::fs::write(&asm_path, "org 100h\nmvi a, 42h\nret\n").unwrap();

        let (report, warning) = CompilerRunner::assemble_file(&asm_path, 0, "");
        assert!(warning.is_none());
        assert!(report.success);
        assert!(report.com_path.is_some());

        let com_path = report.com_path.unwrap();
        let (emu_logs, emu_warn) = CompilerRunner::run_in_emulator(&com_path, 0, "");
        assert!(emu_warn.is_none());
        assert!(!emu_logs.is_empty());

        let _ = std::fs::remove_file(&asm_path);
        let _ = std::fs::remove_file(&com_path);
    }

    #[test]
    fn test_assemble_file_fallback_on_invalid_external_path() {
        let temp_dir = std::env::temp_dir();
        let asm_path = temp_dir.join("test_fallback_toolchain.asm");
        std::fs::write(&asm_path, "org 100h\nmvi a, 10h\nret\n").unwrap();

        let (report, warning) =
            CompilerRunner::assemble_file(&asm_path, 1, "non_existent_folder_xyz/asmx20");
        assert!(warning.is_some());
        let warn_str = warning.unwrap();
        assert!(warn_str.contains("ПРЕДУПРЕЖДЕНИЕ"));
        assert!(warn_str.contains("intel-8080-kit"));
        assert!(report.success);
        assert!(report.com_path.is_some());

        let com_path = report.com_path.unwrap();
        let (emu_logs, emu_warn) =
            CompilerRunner::run_in_emulator(&com_path, 1, "non_existent_folder_xyz/cpmemu");
        assert!(emu_warn.is_some());
        let emu_warn_str = emu_warn.unwrap();
        assert!(emu_warn_str.contains("ПРЕДУПРЕЖДЕНИЕ"));
        assert!(emu_warn_str.contains("intel-8080-kit"));
        assert!(!emu_logs.is_empty());

        let _ = std::fs::remove_file(&asm_path);
        let _ = std::fs::remove_file(&com_path);
    }
}
