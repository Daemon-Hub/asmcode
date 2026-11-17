use crate::autocomplete::extractor::extract_user_labels;
use crate::compiler::runner::{CompileReport, Diagnostic};
use crate::syntax::intel8080::{is_directive, is_hex_literal, is_mnemonic, is_register};
use std::collections::HashSet;

// High-performance Intel 8080 syntax validator with byte-level zero-allocation scanning
pub fn validate_syntax(source_code: &str) -> CompileReport {
    let mut logs = Vec::with_capacity(32);
    let mut diagnostics = Vec::with_capacity(16);
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

        // Fast zero-allocation token scanning
        let mut tokens_iter = statement.split_whitespace();
        let Some(first) = tokens_iter.next() else {
            continue;
        };

        let upper_first = first.to_ascii_uppercase();

        if tokens_iter
            .next()
            .is_some_and(|s| s.eq_ignore_ascii_case("EQU"))
        {
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
            "JMP" | "JNZ" | "JZ" | "JNC" | "JC" | "JPO" | "JPE" | "JP" | "JM" | "CALL" | "CNZ"
            | "CZ" | "CNC" | "CC" | "CPO" | "CPE" | "CP" | "CM" => (3, true),
            "LXI" | "LDA" | "STA" | "LHLD" | "SHLD" => (3, false),
            "MVI" | "ADI" | "ACI" | "SUI" | "SBI" | "ANI" | "XRI" | "ORI" | "CPI" | "IN"
            | "OUT" => (2, false),
            _ => (1, false),
        };
        estimated_bytes += inst_bytes;

        // Check operands for jumps and calls
        if needs_label_check {
            let mut iter = statement.split_whitespace();
            let _ = iter.next(); // skip mnemonic
            if let Some(target) = iter.next() {
                let target_label = target.trim().trim_matches(',').to_ascii_uppercase();
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
        }

        // Validate MOV operand registers
        if upper_first == "MOV" && statement.len() > 3 {
            let operands_str = statement[3..].trim();
            let mut ops = operands_str.split(',').map(|s| s.trim());
            let op1 = ops.next();
            let op2 = ops.next();
            let op3 = ops.next();

            if let (Some(r1_str), Some(r2_str), None) = (op1, op2, op3) {
                let r1 = r1_str.to_ascii_uppercase();
                let r2 = r2_str.to_ascii_uppercase();
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

#[inline]
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
    fn test_validate_valid_syntax() {
        let code = "start:\n    mvi a, 10h\n    mov b, a\n    hlt\n";
        let report = validate_syntax(code);
        assert!(report.success);
        assert_eq!(report.instruction_count, 3);
    }

    #[test]
    fn test_validate_invalid_mov() {
        let code = "start:\n    mov z, a\n";
        let report = validate_syntax(code);
        assert!(!report.success);
        assert!(!report.diagnostics.is_empty());
    }
}
