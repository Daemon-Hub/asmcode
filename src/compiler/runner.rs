use crate::autocomplete::extractor::extract_user_labels;
use crate::syntax::intel8080::{is_directive, is_hex_literal, is_mnemonic, is_register};
use std::collections::HashSet;
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
        let mut logs = Vec::new();
        let mut diagnostics = Vec::new();
        let mut instruction_count = 0;
        let mut estimated_bytes = 0;

        let defined_labels: HashSet<String> = extract_user_labels(source_code)
            .into_iter()
            .map(|l| l.name.to_ascii_uppercase())
            .collect();

        logs.push(format!(
            "[{}] [CHECK] Analyzing Intel 8080 syntax...",
            get_timestamp()
        ));

        for (idx, line) in source_code.lines().enumerate() {
            let line_number = idx + 1;
            let trimmed = line.trim();

            if trimmed.is_empty() || trimmed.starts_with(';') {
                continue;
            }

            let code_without_comment = match trimmed.find(';') {
                Some(pos) => trimmed[..pos].trim(),
                None => trimmed,
            };

            let statement = match code_without_comment.find(':') {
                Some(pos) => code_without_comment[pos + 1..].trim(),
                None => code_without_comment,
            };

            if statement.is_empty() {
                continue;
            }

            let tokens: Vec<&str> = statement.split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }

            let first = tokens[0];
            let upper_first = first.to_ascii_uppercase();

            if tokens.len() > 1 && tokens[1].eq_ignore_ascii_case("EQU") {
                continue;
            }

            if is_directive(&upper_first) {
                match upper_first.as_str() {
                    "DB" => {
                        let operands = statement[2..].trim();
                        let count = operands.split(',').count().max(1);
                        estimated_bytes += count;
                    }
                    "DW" => {
                        let operands = statement[2..].trim();
                        let count = operands.split(',').count().max(1);
                        estimated_bytes += count * 2;
                    }
                    _ => {}
                }
                continue;
            }

            if !is_mnemonic(&upper_first) {
                diagnostics.push(Diagnostic {
                    line: line_number,
                    message: format!("Unknown mnemonic or command '{}'", first),
                    is_error: true,
                });
                continue;
            }

            instruction_count += 1;
            let (inst_bytes, needs_label_check) = match upper_first.as_str() {
                "JMP" | "JNZ" | "JZ" | "JNC" | "JC" | "JPO" | "JPE" | "JP" | "JM" | "CALL"
                | "CNZ" | "CZ" | "CNC" | "CC" | "CPO" | "CPE" | "CP" | "CM" => (3, true),
                "LXI" | "LDA" | "STA" | "LHLD" | "SHLD" => (3, false),
                "MVI" | "ADI" | "ACI" | "SUI" | "SBI" | "ANI" | "XRI" | "ORI" | "CPI" | "IN"
                | "OUT" => (2, false),
                _ => (1, false),
            };
            estimated_bytes += inst_bytes;

            if needs_label_check && tokens.len() > 1 {
                let target_label = tokens[1].trim().trim_matches(',').to_ascii_uppercase();
                if !is_hex_literal(&target_label)
                    && !target_label.chars().all(|c| c.is_ascii_digit())
                    && !defined_labels.contains(&target_label)
                {
                    diagnostics.push(Diagnostic {
                        line: line_number,
                        message: format!("Reference to undefined label '{}'", target_label),
                        is_error: false,
                    });
                }
            }

            if upper_first == "MOV" && statement.len() > 3 {
                let operands_str = statement[3..].trim();
                let operands: Vec<&str> = operands_str.split(',').map(|s| s.trim()).collect();
                if operands.len() == 2 {
                    let r1 = operands[0].to_ascii_uppercase();
                    let r2 = operands[1].to_ascii_uppercase();
                    if !is_register(&r1) {
                        diagnostics.push(Diagnostic {
                            line: line_number,
                            message: format!("Invalid destination register '{}' for MOV", r1),
                            is_error: true,
                        });
                    }
                    if !is_register(&r2) {
                        diagnostics.push(Diagnostic {
                            line: line_number,
                            message: format!("Invalid source register '{}' for MOV", r2),
                            is_error: true,
                        });
                    }
                } else {
                    diagnostics.push(Diagnostic {
                        line: line_number,
                        message: "MOV instruction requires exactly two operands (e.g. MOV A, B)"
                            .to_string(),
                        is_error: true,
                    });
                }
            }
        }

        let has_errors = diagnostics.iter().any(|d| d.is_error);

        for diag in &diagnostics {
            let prefix = if diag.is_error { "ERROR" } else { "WARNING" };
            logs.push(format!(
                "[{}] [Line {:3}] {}: {}",
                prefix, diag.line, prefix, diag.message
            ));
        }

        if has_errors {
            logs.push(format!(
                "[{}] Validation finished with {} error(s).",
                get_timestamp(),
                diagnostics.iter().filter(|d| d.is_error).count()
            ));
        } else {
            logs.push(format!(
                "[{}] Syntax OK: 0 errors, {} instructions, ~{} estimated bytes, {} labels defined.",
                get_timestamp(),
                instruction_count,
                estimated_bytes,
                defined_labels.len()
            ));
        }

        CompileReport {
            success: !has_errors,
            output_logs: logs,
            diagnostics,
            instruction_count,
            estimated_bytes,
            com_path: None,
        }
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
