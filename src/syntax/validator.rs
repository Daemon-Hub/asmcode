use crate::autocomplete::extractor::extract_user_labels;
use crate::compiler::runner::{CompileReport, Diagnostic};
use crate::syntax::intel8080::{
    is_directive, is_hex_literal, is_mnemonic, is_register, pack_mnemonic_u32,
};
use std::collections::HashSet;

// High-performance Intel 8080 syntax validator with byte-level zero-allocation scanning
pub fn validate_syntax(source_code: &str) -> CompileReport {
    let mut logs = Vec::with_capacity(32);
    let mut diagnostics = Vec::with_capacity(16);
    let mut instruction_count = 0;
    let mut estimated_bytes = 0;

    let user_symbols = extract_user_labels(source_code);
    let mut defined_labels: HashSet<String> = HashSet::with_capacity(user_symbols.len());
    for s in user_symbols {
        defined_labels.insert(s.name.to_ascii_uppercase());
    }

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

        if tokens_iter
            .next()
            .is_some_and(|s| s.eq_ignore_ascii_case("EQU"))
        {
            continue;
        }

        if is_directive(first) {
            if first.eq_ignore_ascii_case("DB") {
                let operands = statement[2..].trim();
                let count = operands.split(',').count().max(1);
                estimated_bytes += count;
            } else if first.eq_ignore_ascii_case("DW") {
                let operands = statement[2..].trim();
                let count = operands.split(',').count().max(1);
                estimated_bytes += count * 2;
            }
            continue;
        }

        if !is_mnemonic(first) {
            diagnostics.push(Diagnostic {
                line: line_number,
                message: format!("Unknown mnemonic or command '{}'", first),
                is_error: true,
            });
            continue;
        }

        instruction_count += 1;
        let packed = pack_mnemonic_u32(first.as_bytes());
        let (inst_bytes, needs_label_check) = match &packed.to_le_bytes() {
            b"JMP\0" | b"JNZ\0" | b"JZ\0\0" | b"JNC\0" | b"JC\0\0" | b"JPO\0" | b"JPE\0"
            | b"JP\0\0" | b"JM\0\0" | b"CALL" | b"CNZ\0" | b"CZ\0\0" | b"CNC\0" | b"CC\0\0"
            | b"CPO\0" | b"CPE\0" | b"CP\0\0" | b"CM\0\0" => (3, true),
            b"LXI\0" | b"LDA\0" | b"STA\0" | b"LHLD" | b"SHLD" => (3, false),
            b"MVI\0" | b"ADI\0" | b"ACI\0" | b"SUI\0" | b"SBI\0" | b"ANI\0" | b"XRI\0"
            | b"ORI\0" | b"CPI\0" | b"IN\0\0" | b"OUT\0" => (2, false),
            _ => (1, false),
        };
        estimated_bytes += inst_bytes;

        // Check operands for jumps and calls
        if needs_label_check {
            let mut iter = statement.split_whitespace();
            let _ = iter.next(); // skip mnemonic
            if let Some(target) = iter.next() {
                let clean_target = target.trim().trim_matches(',');
                if !is_hex_literal(clean_target)
                    && !clean_target.bytes().all(|b| b.is_ascii_digit())
                {
                    let target_upper = clean_target.to_ascii_uppercase();
                    if !defined_labels.contains(&target_upper) {
                        diagnostics.push(Diagnostic {
                            line: line_number,
                            message: format!("Reference to undefined label '{}'", target_upper),
                            is_error: false,
                        });
                    }
                }
            }
        }

        // Validate MOV operand registers
        if first.eq_ignore_ascii_case("MOV") && statement.len() > 3 {
            let operands_str = statement[3..].trim();
            let mut ops = operands_str.split(',').map(|s| s.trim());
            let op1 = ops.next();
            let op2 = ops.next();
            let op3 = ops.next();

            if let (Some(r1_str), Some(r2_str), None) = (op1, op2, op3) {
                if !is_register(r1_str) {
                    diagnostics.push(Diagnostic {
                        line: line_number,
                        message: format!("Invalid destination register '{}' for MOV", r1_str),
                        is_error: true,
                    });
                }
                if !is_register(r2_str) {
                    diagnostics.push(Diagnostic {
                        line: line_number,
                        message: format!("Invalid source register '{}' for MOV", r2_str),
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
