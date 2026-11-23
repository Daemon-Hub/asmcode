use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum SymbolKind {
    Code = 0,     // Instruction routine/jump target (start:, sum_loop:, print_done:)
    Data = 1, // Memory buffer/variable definition (msg: db '...', result: db 00h, buffer: ds 64)
    Constant = 2, // Symbolic equate value (TPA equ 100h, BDOS equ 0005h)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UserSymbol {
    pub name: String,
    pub line_number: usize,
    pub kind: SymbolKind,
    pub detail: String, // e.g. "100h", "DB 'Hello...'", "lxi sp, ..."
}

pub fn extract_user_labels(source: &str) -> Vec<UserSymbol> {
    let mut symbols = Vec::with_capacity(32);
    let mut seen_names = HashSet::with_capacity(32);

    for (line_idx, line) in source.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with(';') || trimmed.is_empty() {
            continue;
        }

        let code_part = match trimmed.find(';') {
            Some(idx) => trimmed[..idx].trim(),
            None => trimmed,
        };
        if code_part.is_empty() {
            continue;
        }

        // Case 1: Label with colon (e.g. "start:", "sum_loop: mov a, m", "msg: db '...'")
        if let Some(colon_idx) = code_part.find(':') {
            let label_candidate = code_part[..colon_idx].trim();
            let after_colon = code_part[colon_idx + 1..].trim();

            if is_valid_identifier(label_candidate) {
                let lower = label_candidate.to_ascii_lowercase();
                if seen_names.insert(lower) {
                    let mut after_words = after_colon.split_whitespace();
                    let first_word = after_words.next().unwrap_or("");

                    let (kind, detail) = if first_word.eq_ignore_ascii_case("DB")
                        || first_word.eq_ignore_ascii_case("DW")
                        || first_word.eq_ignore_ascii_case("DS")
                    {
                        (SymbolKind::Data, after_colon.to_string())
                    } else if first_word.eq_ignore_ascii_case("EQU")
                        || first_word.eq_ignore_ascii_case("SET")
                    {
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

                    symbols.push(UserSymbol {
                        name: label_candidate.to_string(),
                        line_number: line_idx + 1,
                        kind,
                        detail,
                    });
                }
            }
        } else {
            // Case 2: Statement without colon (e.g. "TPA equ 100h", "BUFFER ds 64", "TABLE dw 1000h")
            let mut words = code_part.split_whitespace();
            if let (Some(first), Some(second)) = (words.next(), words.next())
                && is_valid_identifier(first)
            {
                let lower = first.to_ascii_lowercase();
                if seen_names.insert(lower) {
                    if second.eq_ignore_ascii_case("EQU") || second.eq_ignore_ascii_case("SET") {
                        let rest = code_part[first.len()..].trim_start();
                        let val = rest[second.len()..].trim();
                        symbols.push(UserSymbol {
                            name: first.to_string(),
                            line_number: line_idx + 1,
                            kind: SymbolKind::Constant,
                            detail: if val.is_empty() {
                                "EQU".to_string()
                            } else {
                                val.to_string()
                            },
                        });
                    } else if second.eq_ignore_ascii_case("DB")
                        || second.eq_ignore_ascii_case("DW")
                        || second.eq_ignore_ascii_case("DS")
                    {
                        let rest = code_part[first.len()..].trim_start();
                        symbols.push(UserSymbol {
                            name: first.to_string(),
                            line_number: line_idx + 1,
                            kind: SymbolKind::Data,
                            detail: rest.to_string(),
                        });
                    }
                }
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
}
