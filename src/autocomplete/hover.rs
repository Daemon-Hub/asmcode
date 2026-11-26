use super::extractor::{SymbolKind, extract_user_labels};
use super::intel8080_details::{get_detailed_code_example, get_detailed_description};
use super::intel8080_docs::INTEL_8080_DOCS;
use crate::docs::registers_psw::REGISTERS_DOCS;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HoverSpan {
    pub line: usize,
    pub start_col: usize,
    pub len_chars: usize,
    pub word: String,
}

#[derive(Clone, Debug)]
pub struct HoverInfo {
    pub span: HoverSpan,
    pub title: String,
    pub category: String,
    pub badge: String,
    pub syntax: String,
    pub flags: String,
    pub summary: String,
    pub example: String,
    pub kind: &'static str,
}

#[inline]
fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// Extracts an assembly identifier at the given 0-based `(line_idx, col_idx)`, ignoring comments, strings, and numeric literals.
pub fn extract_hover_word(source: &str, line_idx: usize, col_idx: usize) -> Option<HoverSpan> {
    let line_str = source.lines().nth(line_idx)?;

    let mut in_single = false;
    let mut in_double = false;
    let mut comment_start_char = None;
    let mut target_ch = None;

    for (ci, ch) in line_str.chars().enumerate() {
        if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == ';' && !in_single && !in_double {
            comment_start_char = Some(ci);
            break;
        }
        if ci == col_idx {
            if in_single || in_double {
                return None;
            }
            target_ch = Some(ch);
        }
    }

    if let Some(c_start) = comment_start_char
        && col_idx >= c_start
    {
        return None;
    }

    let ch = target_ch?;
    if !is_ident_char(ch) {
        return None;
    }

    let chars: Vec<(usize, char)> = line_str.char_indices().collect();
    if col_idx >= chars.len() {
        return None;
    }

    let mut start_col = col_idx;
    while start_col > 0 && is_ident_char(chars[start_col - 1].1) {
        start_col -= 1;
    }

    let mut end_col = col_idx + 1;
    while end_col < chars.len() && is_ident_char(chars[end_col].1) {
        end_col += 1;
    }

    // Numeric literals (e.g. 0FFh, 100, 10) start with an ASCII digit.
    if chars[start_col].1.is_ascii_digit() {
        return None;
    }

    let start_byte = chars[start_col].0;
    let end_byte = if end_col < chars.len() {
        chars[end_col].0
    } else {
        line_str.len()
    };

    Some(HoverSpan {
        line: line_idx,
        start_col,
        len_chars: end_col - start_col,
        word: line_str[start_byte..end_byte].to_string(),
    })
}

fn normalize_example_indent(raw: &str) -> String {
    let mut lines = Vec::new();
    for line in raw.lines() {
        let stripped = line.strip_prefix("        ").unwrap_or(line);
        lines.push(stripped.trim_end());
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines.join("\n")
}

fn strip_inline_comment(line: &str) -> &str {
    let mut in_single = false;
    let mut in_double = false;
    for (idx, ch) in line.char_indices() {
        if ch == '\'' && !in_double {
            in_single = !in_single;
        } else if ch == '"' && !in_single {
            in_double = !in_double;
        } else if ch == ';' && !in_single && !in_double {
            return &line[..idx];
        }
    }
    line
}

fn starts_new_symbol(code: &str) -> bool {
    if let Some(colon_idx) = code.find(':') {
        let candidate = code[..colon_idx].trim();
        if !candidate.is_empty() && candidate.chars().all(is_ident_char) {
            return true;
        }
    }
    let mut words = code.split_whitespace();
    if let (Some(first), Some(second)) = (words.next(), words.next())
        && !first.is_empty()
        && first.chars().all(is_ident_char)
        && (second.eq_ignore_ascii_case("EQU")
            || second.eq_ignore_ascii_case("SET")
            || second.eq_ignore_ascii_case("DB")
            || second.eq_ignore_ascii_case("DW")
            || second.eq_ignore_ascii_case("DS"))
    {
        return true;
    }
    false
}

fn is_data_continuation(code: &str) -> bool {
    let first = code.split_whitespace().next().unwrap_or("");
    first.eq_ignore_ascii_case("DB")
        || first.eq_ignore_ascii_case("DW")
        || first.eq_ignore_ascii_case("DS")
}

fn is_block_terminator(code: &str) -> bool {
    let after_colon = match code.find(':') {
        Some(idx) => code[idx + 1..].trim(),
        None => code,
    };
    let first = after_colon.split_whitespace().next().unwrap_or("");
    first.eq_ignore_ascii_case("RET")
        || first.eq_ignore_ascii_case("JMP")
        || first.eq_ignore_ascii_case("HLT")
        || first.eq_ignore_ascii_case("END")
        || first.eq_ignore_ascii_case("PCHL")
}

fn dedent_block(lines: &[&str]) -> String {
    let min_indent = lines
        .iter()
        .filter(|l| !l.trim().is_empty())
        .map(|l| l.bytes().take_while(|b| *b == b' ' || *b == b'\t').count())
        .min()
        .unwrap_or(0);

    lines
        .iter()
        .map(|l| {
            if l.len() >= min_indent {
                l[min_indent..].trim_end()
            } else {
                l.trim_end()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn extract_definition_context(source: &str, line_number_1_based: usize, kind: SymbolKind) -> String {
    if line_number_1_based == 0 {
        return String::new();
    }
    let start_idx = line_number_1_based - 1;
    let mut lines_iter = source.lines().skip(start_idx);
    let Some(first_line) = lines_iter.next() else {
        return String::new();
    };

    let mut collected = vec![first_line.trim_end()];

    match kind {
        SymbolKind::Constant => {}
        SymbolKind::Data => {
            for line in lines_iter.take(15) {
                let code = strip_inline_comment(line).trim();
                if code.is_empty() || starts_new_symbol(code) || !is_data_continuation(code) {
                    break;
                }
                collected.push(line.trim_end());
            }
        }
        SymbolKind::Code => {
            if !is_block_terminator(strip_inline_comment(first_line).trim()) {
                for line in lines_iter.take(7) {
                    let code = strip_inline_comment(line).trim();
                    if code.is_empty() || starts_new_symbol(code) {
                        break;
                    }
                    collected.push(line.trim_end());
                    if is_block_terminator(code) {
                        break;
                    }
                }
            }
        }
    }

    dedent_block(&collected)
}

fn register_example(name: &str, is_ru: bool) -> String {
    match name {
        "A" => {
            if is_ru {
                "mvi a, 0FFh     ; Загрузка константы в аккумулятор A\nadi 01h         ; Сложение в A (A = A + 1)"
            } else {
                "mvi a, 0FFh     ; Load constant into accumulator A\nadi 01h         ; Arithmetic in A (A = A + 1)"
            }
        }
        "B" => {
            if is_ru {
                "mvi b, 10       ; Инициализация регистра B\ndcr b           ; Декремент счетчика B"
            } else {
                "mvi b, 10       ; Initialize register B\ndcr b           ; Decrement counter B"
            }
        }
        "C" => {
            if is_ru {
                "mvi c, 09h      ; Код функции BDOS (печать строки)\ncall 0005h      ; Вызов CP/M BDOS"
            } else {
                "mvi c, 09h      ; BDOS function number (print string)\ncall 0005h      ; Call CP/M BDOS"
            }
        }
        "D" | "E" => {
            if is_ru {
                "lxi d, msg      ; Адрес строки в паре DE\nldax d          ; Чтение байта по адресу [DE]"
            } else {
                "lxi d, msg      ; String address in DE pair\nldax d          ; Load byte from [DE]"
            }
        }
        "H" | "L" => {
            if is_ru {
                "lxi h, buffer   ; Базовый адрес в паре HL\nmov a, m        ; Чтение байта по адресу [HL]"
            } else {
                "lxi h, buffer   ; Base address in HL pair\nmov a, m        ; Read byte at [HL]"
            }
        }
        "M" => {
            if is_ru {
                "lxi h, buffer   ; Указатель HL на память\nmov a, m        ; Чтение ячейки [HL] в A\ninr m           ; Инкремент ячейки [HL]"
            } else {
                "lxi h, buffer   ; Point HL to memory\nmov a, m        ; Read byte [HL] into A\ninr m           ; Increment memory byte [HL]"
            }
        }
        "SP" => {
            if is_ru {
                "lxi sp, 0FFFFh  ; Инициализация вершины стека\npush h          ; Сохранить HL в стек"
            } else {
                "lxi sp, 0FFFFh  ; Initialize top of stack\npush h          ; Push HL onto stack"
            }
        }
        "PC" => {
            if is_ru {
                "jmp main        ; Загрузить адрес main в PC\npchl            ; PC = HL (косвенный переход)"
            } else {
                "jmp main        ; Load main address into PC\npchl            ; PC = HL (indirect jump)"
            }
        }
        "BC" => {
            if is_ru {
                "lxi b, 1234h    ; Загрузить 16-бит значение в BC\ninx b           ; Инкремент пары BC"
            } else {
                "lxi b, 1234h    ; Load 16-bit value into BC\ninx b           ; Increment BC pair"
            }
        }
        "DE" => {
            if is_ru {
                "lxi d, msg      ; Загрузить адрес msg в DE\nxchg            ; Обменять HL и DE"
            } else {
                "lxi d, msg      ; Load msg address into DE\nxchg            ; Swap HL and DE"
            }
        }
        "HL" => {
            if is_ru {
                "lxi h, table    ; Адрес таблицы в HL\ndad b           ; HL = HL + BC"
            } else {
                "lxi h, table    ; Table address in HL\ndad b           ; HL = HL + BC"
            }
        }
        "PSW" => {
            if is_ru {
                "push psw        ; Сохранить A и флаги в стек\npop psw         ; Восстановить A и флаги"
            } else {
                "push psw        ; Save A and flags on stack\npop psw         ; Restore A and flags"
            }
        }
        _ => "",
    }
    .to_string()
}

pub fn resolve_hover(source: &str, line_idx: usize, col_idx: usize, lang: &str) -> Option<HoverInfo> {
    let span = extract_hover_word(source, line_idx, col_idx)?;
    let word = span.word.as_str();
    let is_ru = lang == "ru";

    // 1. Intel 8080 instructions & assembler directives
    if let Some(doc) = INTEL_8080_DOCS
        .iter()
        .find(|d| d.name.eq_ignore_ascii_case(word))
    {
        let detailed = get_detailed_description(doc.name, lang);
        let summary = if !detailed.is_empty() {
            detailed
        } else {
            doc.get_summary(lang)
        };

        let badge = if doc.bytes > 0 {
            if is_ru {
                format!("{} такт. · {} Б", doc.cycles, doc.bytes)
            } else {
                format!("{} cyc · {} B", doc.cycles, doc.bytes)
            }
        } else {
            String::new()
        };

        let detailed_ex = normalize_example_indent(get_detailed_code_example(doc.name));
        let example = if !detailed_ex.is_empty() {
            detailed_ex
        } else {
            doc.get_example().to_string()
        };

        return Some(HoverInfo {
            span,
            title: doc.name.to_string(),
            category: doc.get_category(lang).to_string(),
            badge,
            syntax: doc.syntax.to_string(),
            flags: doc.flags.to_string(),
            summary: summary.to_string(),
            example,
            kind: doc.get_icon_kind(),
        });
    }

    // 2. Intel 8080 registers & register pairs
    if let Some(reg) = REGISTERS_DOCS
        .iter()
        .find(|r| r.name.eq_ignore_ascii_case(word))
    {
        let category = if is_ru {
            format!("Регистр · {} бит", reg.bits)
        } else {
            format!("{}-bit Register", reg.bits)
        };
        let role = if is_ru { reg.role_ru } else { reg.role_en };
        let desc = if is_ru { reg.desc_ru } else { reg.desc_en };

        let pair_name = if is_ru {
            reg.pair_name_ru
        } else {
            reg.pair_name_en
        };

        return Some(HoverInfo {
            span,
            title: reg.name.to_string(),
            category,
            badge: pair_name.to_string(),
            syntax: role.to_string(),
            flags: String::new(),
            summary: desc.to_string(),
            example: register_example(reg.name, is_ru),
            kind: "register",
        });
    }

    // 3. User-defined labels, constants (EQU), and data buffers (DB/DW/DS)
    let user_symbols = extract_user_labels(source);
    if let Some(sym) = user_symbols
        .iter()
        .find(|s| s.name.eq_ignore_ascii_case(word))
    {
        let badge = if is_ru {
            format!("Строка {}", sym.line_number)
        } else {
            format!("Line {}", sym.line_number)
        };
        let (category, syntax, summary) = match sym.kind {
            SymbolKind::Constant => (
                if is_ru {
                    "Константа (EQU)".to_string()
                } else {
                    "Constant (EQU)".to_string()
                },
                format!("{} equ {}", sym.name, sym.detail),
                if is_ru {
                    format!(
                        "Символьная константа (строка {}): {}",
                        sym.line_number, sym.detail
                    )
                } else {
                    format!(
                        "Symbolic constant defined on line {}: {}",
                        sym.line_number, sym.detail
                    )
                },
            ),
            SymbolKind::Data => (
                if is_ru {
                    "Данные / Буфер".to_string()
                } else {
                    "Data / Buffer".to_string()
                },
                format!("{}: {}", sym.name, sym.detail),
                if is_ru {
                    format!("Определение данных в памяти (строка {}).", sym.line_number)
                } else {
                    format!("Memory data definition on line {}.", sym.line_number)
                },
            ),
            SymbolKind::Code => (
                if is_ru {
                    "Метка кода".to_string()
                } else {
                    "Code Label".to_string()
                },
                format!("{}:", sym.name),
                if is_ru {
                    format!(
                        "Метка перехода / подпрограмма (строка {}): {}",
                        sym.line_number, sym.detail
                    )
                } else {
                    format!(
                        "Jump target / subroutine label (line {}): {}",
                        sym.line_number, sym.detail
                    )
                },
            ),
        };

        return Some(HoverInfo {
            span,
            title: sym.name.clone(),
            category,
            badge,
            syntax,
            flags: String::new(),
            summary,
            example: extract_definition_context(source, sym.line_number, sym.kind),
            kind: "label",
        });
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_hover_word_skips_comments_strings_and_numbers() {
        let src = "BDOS equ 0005h\nstart:\n    mvi c, 09h ; print string\n    db 'mvi a, b'\n";

        // Hover over "mvi" on line 2
        let w = extract_hover_word(src, 2, 5).unwrap();
        assert_eq!(w.word, "mvi");
        assert_eq!(w.start_col, 4);
        assert_eq!(w.len_chars, 3);

        // Hover over numeric "09h" on line 2 -> ignored
        assert!(extract_hover_word(src, 2, 11).is_none());

        // Hover inside comment "; print string" -> ignored
        assert!(extract_hover_word(src, 2, 18).is_none());

        // Hover inside string "'mvi a, b'" -> ignored
        assert!(extract_hover_word(src, 3, 9).is_none());
    }

    #[test]
    fn test_resolve_hover_instruction_register_and_label() {
        let src = "BDOS equ 0005h\nstart:\n    mvi c, 09h\n    call BDOS\n";

        let ins = resolve_hover(src, 2, 4, "en").unwrap();
        assert_eq!(ins.title, "MVI");
        assert_eq!(ins.kind, "transfer");

        let reg = resolve_hover(src, 2, 8, "ru").unwrap();
        assert_eq!(reg.title, "C");
        assert_eq!(reg.kind, "register");

        let sym = resolve_hover(src, 3, 10, "ru").unwrap();
        assert_eq!(sym.title, "BDOS");
        assert_eq!(sym.kind, "label");
        assert!(sym.syntax.contains("0005h"));
    }

    #[test]
    fn test_dynamic_definition_context_bounds() {
        let src = "\
BDOS    equ 0005h
PRINTF  equ 09h
READF   equ 0Ah

    org 100h
start:
    call helper
    lxi d, buffer
    lxi h, single_val
    ret

helper:
    mvi a, 01h
    ret
other_label:
    nop

buffer: db 20    ; max
        db 0     ; len
        ds 20    ; buf
single_val: db 42
";
        let equ_bdos = resolve_hover(src, 0, 1, "ru").unwrap();
        assert_eq!(equ_bdos.example, "BDOS    equ 0005h");

        let equ_readf = resolve_hover(src, 2, 1, "ru").unwrap();
        assert_eq!(equ_readf.example, "READF   equ 0Ah");

        let buf = resolve_hover(src, 7, 12, "ru").unwrap();
        assert_eq!(
            buf.example,
            "buffer: db 20    ; max\n        db 0     ; len\n        ds 20    ; buf"
        );

        let single = resolve_hover(src, 8, 12, "ru").unwrap();
        assert_eq!(single.example, "single_val: db 42");

        let helper = resolve_hover(src, 6, 10, "ru").unwrap();
        assert_eq!(helper.example, "helper:\n    mvi a, 01h\n    ret");
    }
}
