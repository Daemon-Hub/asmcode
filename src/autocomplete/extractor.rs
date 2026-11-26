use rayon::prelude::*;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SymbolKind {
    Code = 0,     // Instruction routine/jump target (start:, sum_loop:, print_done:)
    Data = 1,     // Memory buffer/variable definition (msg: db '...', result: db 00h, buffer: ds 64)
    Constant = 2, // Symbolic equate value (TPA equ 100h, BDOS equ 0005h)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserSymbol {
    pub name: String,
    pub line_number: usize,
    pub kind: SymbolKind,
    pub detail: String, // e.g. "100h", "DB 'Hello...'", "lxi sp, ..."
}

#[inline]
fn parse_line_symbol(line_idx: usize, line: &str) -> Option<UserSymbol> {
    let trimmed = line.trim();
    if trimmed.is_empty() || trimmed.starts_with(';') {
        return None;
    }

    let code_part = match trimmed.find(';') {
        Some(idx) => trimmed[..idx].trim(),
        None => trimmed,
    };
    if code_part.is_empty() {
        return None;
    }

    // Case 1: Label with colon (e.g. "start:", "sum_loop: mov a, m", "msg: db '...'")
    if let Some(colon_idx) = code_part.find(':') {
        let label_candidate = code_part[..colon_idx].trim();
        if !is_valid_identifier(label_candidate) {
            return None;
        }
        let after_colon = code_part[colon_idx + 1..].trim();
        let first_word = after_colon.split_whitespace().next().unwrap_or("");

        let (kind, detail) = if first_word.eq_ignore_ascii_case("DB")
            || first_word.eq_ignore_ascii_case("DW")
            || first_word.eq_ignore_ascii_case("DS")
        {
            (SymbolKind::Data, after_colon.to_string())
        } else if first_word.eq_ignore_ascii_case("EQU") || first_word.eq_ignore_ascii_case("SET") {
            let val = after_colon[first_word.len()..].trim();
            (SymbolKind::Constant, val.to_string())
        } else {
            (
                SymbolKind::Code,
                if after_colon.is_empty() {
                    "Подпрограмма / метка".to_string()
                } else {
                    after_colon.to_string()
                },
            )
        };

        return Some(UserSymbol {
            name: label_candidate.to_string(),
            line_number: line_idx + 1,
            kind,
            detail,
        });
    }

    // Case 2: Statement without colon (e.g. "TPA equ 100h", "BUFFER ds 64", "TABLE dw 1000h")
    let mut words = code_part.split_whitespace();
    let (Some(first), Some(second)) = (words.next(), words.next()) else {
        return None;
    };
    if !is_valid_identifier(first) {
        return None;
    }

    if second.eq_ignore_ascii_case("EQU") || second.eq_ignore_ascii_case("SET") {
        let rest = code_part[first.len()..].trim_start();
        let val = rest[second.len()..].trim();
        return Some(UserSymbol {
            name: first.to_string(),
            line_number: line_idx + 1,
            kind: SymbolKind::Constant,
            detail: if val.is_empty() {
                "EQU".to_string()
            } else {
                val.to_string()
            },
        });
    }

    if second.eq_ignore_ascii_case("DB")
        || second.eq_ignore_ascii_case("DW")
        || second.eq_ignore_ascii_case("DS")
    {
        let rest = code_part[first.len()..].trim_start();
        return Some(UserSymbol {
            name: first.to_string(),
            line_number: line_idx + 1,
            kind: SymbolKind::Data,
            detail: rest.to_string(),
        });
    }

    None
}

pub fn extract_user_labels(source: &str) -> Vec<UserSymbol> {
    extract_user_labels_with_starts(source, None)
}

pub fn extract_user_labels_with_starts(
    source: &str,
    line_starts: Option<&[usize]>,
) -> Vec<UserSymbol> {
    if source.is_empty() {
        return Vec::new();
    }

    const PAR_BYTES_THRESHOLD: usize = 64 * 1024;
    const LINES_PER_CHUNK: usize = 1024;

    if source.len() < PAR_BYTES_THRESHOLD && line_starts.is_none_or(|s| s.len() < LINES_PER_CHUNK) {
        let mut symbols = Vec::with_capacity(32);
        let mut seen_names = HashSet::with_capacity(32);
        for (line_idx, line) in source.lines().enumerate() {
            if let Some(sym) = parse_line_symbol(line_idx, line)
                && seen_names.insert(sym.name.to_ascii_lowercase())
            {
                symbols.push(sym);
            }
        }
        return symbols;
    }

    let owned_starts;
    let starts: &[usize] = match line_starts {
        Some(s) if !s.is_empty() => s,
        _ => {
            owned_starts = crate::syntax::intel8080::build_line_starts(source);
            &owned_starts
        }
    };

    let line_count = starts.len();
    let chunk_indices: Vec<usize> = (0..line_count).step_by(LINES_PER_CHUNK).collect();
    let chunk_results: Vec<Vec<UserSymbol>> = chunk_indices
        .into_par_iter()
        .map(|start_line| {
            let end_line = (start_line + LINES_PER_CHUNK).min(line_count);
            let mut local = Vec::new();
            for line_idx in start_line..end_line {
                let start_b = starts[line_idx];
                let end_b = if line_idx + 1 < line_count {
                    starts[line_idx + 1]
                } else {
                    source.len()
                };
                if start_b >= end_b {
                    continue;
                }
                let raw = source[start_b..end_b]
                    .strip_suffix('\n')
                    .unwrap_or(&source[start_b..end_b]);
                let clean = raw.strip_suffix('\r').unwrap_or(raw);
                if let Some(sym) = parse_line_symbol(line_idx, clean) {
                    local.push(sym);
                }
            }
            local
        })
        .collect();

    let total_est: usize = chunk_results.iter().map(Vec::len).sum();
    let mut symbols = Vec::with_capacity(total_est);
    let mut seen_names = HashSet::with_capacity(total_est);
    for chunk in chunk_results {
        for sym in chunk {
            if seen_names.insert(sym.name.to_ascii_lowercase()) {
                symbols.push(sym);
            }
        }
    }
    symbols
}

#[inline(always)]
fn is_valid_identifier(s: &str) -> bool {
    let bytes = s.as_bytes();
    let Some(&first) = bytes.first() else {
        return false;
    };
    let is_first_valid =
        first.is_ascii_alphabetic() || first == b'_' || first == b'@' || first == b'?';
    if !is_first_valid {
        return false;
    }

    bytes[1..]
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || b == b'_' || b == b'@' || b == b'?')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_label_extraction() {
        let asm = "\
        org 100h
TPA     equ 100h
BDOS    equ 0005h
start:  mvi a, 1
loop_1: dcr a
        jnz loop_1
msg:    db 'Hello, World!\n$'
buffer  ds 64
        hlt";
        let symbols = extract_user_labels(asm);
        assert_eq!(symbols.len(), 6);
        assert_eq!(symbols[0].name, "TPA");
        assert_eq!(symbols[0].kind, SymbolKind::Constant);
        assert_eq!(symbols[1].name, "BDOS");
        assert_eq!(symbols[1].kind, SymbolKind::Constant);
        assert_eq!(symbols[2].name, "start");
        assert_eq!(symbols[2].kind, SymbolKind::Code);
        assert_eq!(symbols[3].name, "loop_1");
        assert_eq!(symbols[3].kind, SymbolKind::Code);
        assert_eq!(symbols[4].name, "msg");
        assert_eq!(symbols[4].kind, SymbolKind::Data);
        assert_eq!(symbols[5].name, "buffer");
        assert_eq!(symbols[5].kind, SymbolKind::Data);
    }

    #[test]
    fn test_parallel_label_extraction_large_buffer() {
        let mut big = String::with_capacity(140 * 1024);
        big.push_str("head_lbl: nop\n");
        for i in 0..4500 {
            if i == 2500 {
                big.push_str("mid_const equ 42h\n");
            }
            big.push_str("        mov a, b\n        adi 01h\n");
        }
        big.push_str("tail_lbl: hlt\n");
        assert!(big.len() > 100 * 1024);

        let symbols = extract_user_labels(&big);
        assert_eq!(symbols.len(), 3);
        assert_eq!(symbols[0].name, "head_lbl");
        assert_eq!(symbols[1].name, "mid_const");
        assert_eq!(symbols[2].name, "tail_lbl");
    }
}
