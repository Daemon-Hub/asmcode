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
    pub fn get_asmx_path() -> PathBuf {
        let fixed_path = PathBuf::from(r"C:\bin\asmx20.exe");
        if fixed_path.exists() {
            return fixed_path;
        }
        PathBuf::from("asmx20.exe")
    }

    pub fn get_cpmemu_path() -> PathBuf {
        let fixed_path = PathBuf::from(r"C:\bin\cpmemu.exe");
        if fixed_path.exists() {
            return fixed_path;
        }
        PathBuf::from("cpmemu.exe")
    }

    pub fn assemble_file(file_path: &Path) -> CompileReport {
        let mut logs = Vec::new();
        let asmx_bin = Self::get_asmx_path();
        let com_path = file_path.with_extension("com");

        logs.push(format!(
            "[{}] [ASSEMBLER] Starting assembly for: {}",
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

        let mut cmd = Command::new(&asmx_bin);
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
                    "[{}] [ERROR] Failed to execute assembler ({}): {}",
                    get_timestamp(),
                    asmx_bin.display(),
                    err
                ));
                return CompileReport {
                    success: false,
                    output_logs: logs,
                    diagnostics: vec![Diagnostic {
                        line: 1,
                        message: format!("Failed to run assembler: {}", err),
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
                "[{}] [SUCCESS] Generated binary: {} ({} bytes)",
                get_timestamp(),
                com_path.file_name().unwrap_or_default().to_string_lossy(),
                file_size
            ));
        } else {
            logs.push(format!(
                "[{}] [ERROR] Assembly failed with exit code: {:?}",
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

    pub fn run_in_emulator(com_path: &Path) -> Vec<String> {
        let mut logs = Vec::new();
        let cpmemu_bin = Self::get_cpmemu_path();

        logs.push(format!(
            "[{}] [EMULATOR] Running in CP/M 8080 Emulator: {}",
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

        let mut cmd = Command::new(&cpmemu_bin);
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
                    "[{}] [EMULATOR] Process finished with exit code {:?}",
                    get_timestamp(),
                    output.status.code()
                ));
            }
            Err(err) => {
                logs.push(format!(
                    "[{}] [ERROR] Failed to run emulator ({}): {}",
                    get_timestamp(),
                    cpmemu_bin.display(),
                    err
                ));
            }
        }

        logs
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
