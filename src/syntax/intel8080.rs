use rayon::prelude::*;
use slint::{Color, SharedString};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TokenType {
    MnemonicDataTransfer = 0,
    MnemonicBranch = 1,
    MnemonicArithmetic = 2,
    MnemonicSystem = 3,
    DirectiveOrg = 4,
    DirectiveEnd = 5,
    DirectiveDef = 6,
    DirectiveData = 7,
    Register8 = 8,
    Register16 = 9,
    ConstantDef = 10,
    Label = 11,
    Identifier = 12,
    NumberHex = 13,
    NumberDec = 14,
    StringLiteral = 15,
    Comment = 16,
    Punctuation = 17,
    Text = 18,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct HighlightToken {
    pub text: SharedString,
    pub token_type: TokenType,
    pub color: Color,
    pub is_italic: bool,
    pub char_count: i32,
    pub is_whitespace: bool,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct HighlightLine {
    pub line_number: i32,
    pub raw_text: String,
    pub tokens: Vec<HighlightToken>,
}

#[derive(Clone, Copy, Debug)]
pub struct SyntaxPalette {
    pub mnemonic_data: Color,
    pub mnemonic_branch: Color,
    pub mnemonic_arith: Color,
    pub mnemonic_system: Color,
    pub directive_org: Color,
    pub directive_end: Color,
    pub directive_def: Color,
    pub directive_data: Color,
    pub register_8: Color,
    pub register_16: Color,
    pub constant_def: Color,
    pub label: Color,
    pub identifier: Color,
    pub number_hex: Color,
    pub number_dec: Color,
    pub string: Color,
    pub comment: Color,
    pub punct: Color,
    pub text: Color,
}

pub const PALETTE_CATPPUCCIN: SyntaxPalette = SyntaxPalette {
    mnemonic_data: Color::from_rgb_u8(137, 180, 250), // #89b4fa sapphire blue
    mnemonic_branch: Color::from_rgb_u8(203, 166, 247), // #cba6f7 mauve
    mnemonic_arith: Color::from_rgb_u8(166, 227, 161), // #a6e3a1 green
    mnemonic_system: Color::from_rgb_u8(243, 139, 168), // #f38ba8 red
    directive_org: Color::from_rgb_u8(243, 139, 168), // #f38ba8 red
    directive_end: Color::from_rgb_u8(203, 166, 247), // #cba6f7 mauve
    directive_def: Color::from_rgb_u8(180, 190, 254), // #b4befe lavender
    directive_data: Color::from_rgb_u8(235, 160, 172), // #eba0ac flamingo
    register_8: Color::from_rgb_u8(148, 226, 213),    // #94e2d5 teal
    register_16: Color::from_rgb_u8(116, 199, 236),   // #74c7ec sky
    constant_def: Color::from_rgb_u8(249, 226, 175),  // #f9e2af yellow
    label: Color::from_rgb_u8(250, 179, 135),         // #fab387 peach
    identifier: Color::from_rgb_u8(137, 220, 235),    // #89dceb sky blue
    number_hex: Color::from_rgb_u8(250, 179, 135),    // #fab387 peach
    number_dec: Color::from_rgb_u8(250, 179, 135),    // #fab387 peach
    string: Color::from_rgb_u8(166, 227, 161),        // #a6e3a1 green
    comment: Color::from_rgb_u8(108, 112, 134),       // #6c7086 overlay0
    punct: Color::from_rgb_u8(147, 153, 178),         // #9399b2 subtext0
    text: Color::from_rgb_u8(137, 220, 235),          // #89dceb sky blue
};

pub const PALETTE_CYBERPUNK: SyntaxPalette = SyntaxPalette {
    mnemonic_data: Color::from_rgb_u8(127, 238, 242), // #7feef2 electric cyan (MOV, MVI, LXI)
    mnemonic_branch: Color::from_rgb_u8(194, 142, 240), // #c28ef0 neon violet (CALL, RET, JMP)
    mnemonic_arith: Color::from_rgb_u8(86, 212, 147), // #56d493 acid green (ADD, INR, DCR)
    mnemonic_system: Color::from_rgb_u8(255, 118, 172), // #ff76ac coral pink (HLT, NOP, EI, DI)
    directive_org: Color::from_rgb_u8(255, 107, 157), // #ff6b9d neon magenta (ORG) - UNIQUE!
    directive_end: Color::from_rgb_u8(162, 155, 254), // #a29bfe soft purple (END) - UNIQUE!
    directive_def: Color::from_rgb_u8(179, 136, 255), // #b388ff light violet (EQU, SET) - UNIQUE!
    directive_data: Color::from_rgb_u8(224, 108, 117), // #e06c75 rose coral (DB, DW, DS)
    register_8: Color::from_rgb_u8(172, 209, 69),     // #acd145 acid lime green (a, b, c, d)
    register_16: Color::from_rgb_u8(43, 203, 186),    // #2bcbba bright mint (SP, PSW, BC, DE, HL)
    constant_def: Color::from_rgb_u8(248, 233, 93), // #f8e95d electric yellow (TPA, BDOS, WSTRF in EQU) - UNIQUE!
    label: Color::from_rgb_u8(255, 167, 28),        // #ffa71c neon amber (msg:) - UNIQUE!
    identifier: Color::from_rgb_u8(72, 219, 251), // #48dbfb bright azure (operands: TPA, msg, BDOS) - UNIQUE!
    number_hex: Color::from_rgb_u8(253, 109, 109), // #fd6d6d neon coral red (100h, 0005h)
    number_dec: Color::from_rgb_u8(255, 159, 67), // #ff9f43 vibrant tangerine (10, 42)
    string: Color::from_rgb_u8(143, 234, 238),    // #8feaee electric mint string
    comment: Color::from_rgb_u8(109, 111, 123),   // #6d6f7b muted slate
    punct: Color::from_rgb_u8(149, 152, 166),     // #9598a6 soft lavender-slate
    text: Color::from_rgb_u8(123, 178, 224),      // #7bb2e0 muted sky blue
};

#[inline(always)]
pub fn get_palette(theme_id: i32) -> &'static SyntaxPalette {
    if theme_id == 1 {
        &PALETTE_CYBERPUNK
    } else {
        &PALETTE_CATPPUCCIN
    }
}

// 256-bit character class bitmap for zero-branch classification
const CC_SPACE: u8 = 1 << 0;
const CC_IDENT: u8 = 1 << 1;
const CC_HEX: u8 = 1 << 2;
const CC_DEC: u8 = 1 << 3;
const CC_PUNCT: u8 = 1 << 4;

static CHAR_MAP: [u8; 256] = {
    let mut map = [0u8; 256];
    let mut i = 0usize;
    while i < 256 {
        let b = i as u8;
        if b == b' ' || b == b'\t' || b == b'\r' || b == b'\n' {
            map[i] |= CC_SPACE;
        }
        if (b >= b'a' && b <= b'z')
            || (b >= b'A' && b <= b'Z')
            || b == b'_'
            || b == b'@'
            || b == b'?'
            || (b >= b'0' && b <= b'9')
        {
            map[i] |= CC_IDENT;
        }
        if (b >= b'0' && b <= b'9') || (b >= b'a' && b <= b'f') || (b >= b'A' && b <= b'F') {
            map[i] |= CC_HEX;
        }
        if b >= b'0' && b <= b'9' {
            map[i] |= CC_DEC;
        }
        if b == b','
            || b == b':'
            || b == b'+'
            || b == b'-'
            || b == b'('
            || b == b')'
            || b == b'['
            || b == b']'
        {
            map[i] |= CC_PUNCT;
        }
        i += 1;
    }
    map
};

#[inline(always)]
const fn pack4(b: &[u8; 4]) -> u32 {
    u32::from_le_bytes(*b)
}

#[inline(always)]
pub fn pack_mnemonic_u32(bytes: &[u8]) -> u32 {
    let mut buf = [0u8; 4];
    let len = bytes.len().min(4);
    let mut i = 0;
    while i < len {
        buf[i] = bytes[i].to_ascii_uppercase();
        i += 1;
    }
    u32::from_le_bytes(buf)
}

#[inline(always)]
pub fn is_mnemonic(word: &str) -> bool {
    classify_mnemonic(word).is_some()
}

#[inline(always)]
pub fn classify_mnemonic(word: &str) -> Option<TokenType> {
    let bytes = word.as_bytes();
    let len = bytes.len();
    if !(2..=4).contains(&len) {
        return None;
    }
    let packed = pack_mnemonic_u32(bytes);

    match &packed.to_le_bytes() {
        b"JMP\0" | b"JC\0\0" | b"JNC\0" | b"JZ\0\0" | b"JNZ\0" | b"JP\0\0" | b"JM\0\0"
        | b"JPE\0" | b"JPO\0" | b"CALL" | b"CC\0\0" | b"CNC\0" | b"CZ\0\0" | b"CNZ\0"
        | b"CP\0\0" | b"CM\0\0" | b"CPE\0" | b"CPO\0" | b"RET\0" | b"RC\0\0" | b"RNC\0"
        | b"RZ\0\0" | b"RNZ\0" | b"RP\0\0" | b"RM\0\0" | b"RPE\0" | b"RPO\0" | b"RST\0"
        | b"PCHL" => Some(TokenType::MnemonicBranch),

        b"ADD\0" | b"ADI\0" | b"ADC\0" | b"ACI\0" | b"SUB\0" | b"SUI\0" | b"SBB\0" | b"SBI\0"
        | b"INR\0" | b"INX\0" | b"DCR\0" | b"DCX\0" | b"DAD\0" | b"DAA\0" | b"ANA\0" | b"ANI\0"
        | b"ORA\0" | b"ORI\0" | b"XRA\0" | b"XRI\0" | b"CMP\0" | b"CPI\0" | b"RLC\0" | b"RRC\0"
        | b"RAL\0" | b"RAR\0" | b"CMA\0" | b"CMC\0" | b"STC\0" => {
            Some(TokenType::MnemonicArithmetic)
        }

        b"HLT\0" | b"NOP\0" | b"EI\0\0" | b"DI\0\0" | b"IN\0\0" | b"OUT\0" | b"SPHL" | b"XTHL" => {
            Some(TokenType::MnemonicSystem)
        }

        b"MOV\0" | b"MVI\0" | b"LXI\0" | b"LDA\0" | b"STA\0" | b"LHLD" | b"SHLD" | b"LDAX"
        | b"STAX" | b"XCHG" | b"PUSH" | b"POP\0" => Some(TokenType::MnemonicDataTransfer),

        _ => None,
    }
}

#[inline(always)]
pub fn is_directive(word: &str) -> bool {
    classify_directive(word).is_some()
}

#[inline(always)]
pub fn classify_directive(word: &str) -> Option<TokenType> {
    let bytes = word.as_bytes();
    let len = bytes.len();
    if !(2..=4).contains(&len) {
        return None;
    }
    let packed = pack_mnemonic_u32(bytes);

    match &packed.to_le_bytes() {
        b"ORG\0" => Some(TokenType::DirectiveOrg),
        b"END\0" => Some(TokenType::DirectiveEnd),
        b"EQU\0" | b"SET\0" => Some(TokenType::DirectiveDef),
        b"DB\0\0" | b"DW\0\0" | b"DS\0\0" => Some(TokenType::DirectiveData),
        _ => None,
    }
}

#[inline(always)]
pub fn is_register(word: &str) -> bool {
    classify_register(word).is_some()
}

#[inline(always)]
pub fn classify_register(word: &str) -> Option<TokenType> {
    let bytes = word.as_bytes();
    if bytes.len() == 1 {
        let b = bytes[0] | 0x20;
        if matches!(b, b'a' | b'b' | b'c' | b'd' | b'e' | b'h' | b'l' | b'm') {
            Some(TokenType::Register8)
        } else {
            None
        }
    } else if bytes.len() == 2 {
        let packed = pack_mnemonic_u32(bytes);
        if packed == pack4(b"SP\0\0")
            || packed == pack4(b"BC\0\0")
            || packed == pack4(b"DE\0\0")
            || packed == pack4(b"HL\0\0")
            || packed == pack4(b"PC\0\0")
        {
            Some(TokenType::Register16)
        } else {
            None
        }
    } else if bytes.len() == 3 {
        let packed = pack_mnemonic_u32(bytes);
        if packed == pack4(b"PSW\0") {
            Some(TokenType::Register16)
        } else {
            None
        }
    } else {
        None
    }
}

#[inline(always)]
pub fn is_hex_literal(word: &str) -> bool {
    let bytes = word.as_bytes();
    let len = bytes.len();
    if len < 2 {
        return false;
    }

    if bytes[0] == b'0' && (bytes[1] == b'x' || bytes[1] == b'X') {
        let mut i = 2;
        while i < len {
            if (CHAR_MAP[bytes[i] as usize] & CC_HEX) == 0 {
                return false;
            }
            i += 1;
        }
        return len > 2;
    }

    if bytes[0] == b'$' {
        let mut i = 1;
        while i < len {
            if (CHAR_MAP[bytes[i] as usize] & CC_HEX) == 0 {
                return false;
            }
            i += 1;
        }
        return len > 1;
    }

    let last = bytes[len - 1] | 0x20;
    if last == b'h' {
        let mut i = 0;
        while i < len - 1 {
            if (CHAR_MAP[bytes[i] as usize] & CC_HEX) == 0 {
                return false;
            }
            i += 1;
        }
        return len > 1;
    }

    false
}

#[inline(always)]
pub fn is_dec_literal(word: &str) -> bool {
    let bytes = word.as_bytes();
    if bytes.is_empty() {
        return false;
    }
    let mut i = 0;
    let len = bytes.len();
    let last = bytes[len - 1] | 0x20;
    let check_len = if last == b'd' { len - 1 } else { len };
    if check_len == 0 {
        return false;
    }
    while i < check_len {
        if (CHAR_MAP[bytes[i] as usize] & CC_DEC) == 0 {
            return false;
        }
        i += 1;
    }
    true
}

pub fn parse_line(line: &str, line_number: i32, theme_id: i32) -> HighlightLine {
    let pal = get_palette(theme_id);
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(16);
    let mut i = 0;

    while i < len {
        let b = bytes[i];

        if b == b';' {
            let comment_text = &line[i..];
            tokens.push(HighlightToken {
                text: SharedString::from(comment_text),
                token_type: TokenType::Comment,
                color: pal.comment,
                is_italic: false,
                char_count: comment_text.chars().count() as i32,
                is_whitespace: false,
            });
            break;
        }

        if b == b'\'' || b == b'"' {
            let quote = b;
            let start = i;
            i += 1;
            while i < len && bytes[i] != quote {
                i += 1;
            }
            if i < len {
                i += 1;
            }
            let str_val = &line[start..i];
            tokens.push(HighlightToken {
                text: SharedString::from(str_val),
                token_type: TokenType::StringLiteral,
                color: pal.string,
                is_italic: false,
                char_count: str_val.chars().count() as i32,
                is_whitespace: false,
            });
            continue;
        }

        if (CHAR_MAP[b as usize] & CC_SPACE) != 0 {
            let start = i;
            let mut char_count = 0;
            while i < len && (CHAR_MAP[bytes[i] as usize] & CC_SPACE) != 0 {
                if bytes[i] == b'\t' {
                    char_count += 4;
                } else {
                    char_count += 1;
                }
                i += 1;
            }
            tokens.push(HighlightToken {
                text: SharedString::from(&line[start..i]),
                token_type: TokenType::Text,
                color: pal.text,
                is_italic: false,
                char_count,
                is_whitespace: true,
            });
            continue;
        }

        if (CHAR_MAP[b as usize] & CC_PUNCT) != 0 {
            tokens.push(HighlightToken {
                text: SharedString::from(&line[i..=i]),
                token_type: TokenType::Punctuation,
                color: pal.punct,
                is_italic: false,
                char_count: 1,
                is_whitespace: false,
            });
            i += 1;
            continue;
        }

        let start = i;
        while i < len
            && (CHAR_MAP[bytes[i] as usize] & (CC_SPACE | CC_PUNCT)) == 0
            && bytes[i] != b';'
            && bytes[i] != b'\''
            && bytes[i] != b'"'
        {
            i += 1;
        }
        let word = &line[start..i];

        if i < len && bytes[i] == b':' {
            tokens.push(HighlightToken {
                text: SharedString::from(word),
                token_type: TokenType::Label,
                color: pal.label,
                is_italic: false,
                char_count: word.chars().count() as i32,
                is_whitespace: false,
            });
            tokens.push(HighlightToken {
                text: SharedString::from(":"),
                token_type: TokenType::Punctuation,
                color: pal.punct,
                is_italic: false,
                char_count: 1,
                is_whitespace: false,
            });
            i += 1;
            continue;
        }

        let (token_type, color, is_italic) = if let Some(m_type) = classify_mnemonic(word) {
            let color = match m_type {
                TokenType::MnemonicDataTransfer => pal.mnemonic_data,
                TokenType::MnemonicBranch => pal.mnemonic_branch,
                TokenType::MnemonicArithmetic => pal.mnemonic_arith,
                _ => pal.mnemonic_system,
            };
            (m_type, color, true)
        } else if let Some(d_type) = classify_directive(word) {
            let color = match d_type {
                TokenType::DirectiveOrg => pal.directive_org,
                TokenType::DirectiveEnd => pal.directive_end,
                TokenType::DirectiveDef => pal.directive_def,
                _ => pal.directive_data,
            };
            (d_type, color, true)
        } else if let Some(r_type) = classify_register(word) {
            let color = match r_type {
                TokenType::Register8 => pal.register_8,
                _ => pal.register_16,
            };
            (r_type, color, true)
        } else if is_hex_literal(word) {
            (TokenType::NumberHex, pal.number_hex, false)
        } else if is_dec_literal(word) {
            (TokenType::NumberDec, pal.number_dec, false)
        } else {
            let mut j = i;
            while j < len && (CHAR_MAP[bytes[j] as usize] & CC_SPACE) != 0 {
                j += 1;
            }
            let is_equ_or_set = j + 3 <= len
                && (((bytes[j] | 0x20 == b'e')
                    && (bytes[j + 1] | 0x20 == b'q')
                    && (bytes[j + 2] | 0x20 == b'u'))
                    || ((bytes[j] | 0x20 == b's')
                        && (bytes[j + 1] | 0x20 == b'e')
                        && (bytes[j + 2] | 0x20 == b't')))
                && (j + 3 == len || (CHAR_MAP[bytes[j + 3] as usize] & CC_SPACE) != 0);
            if is_equ_or_set {
                (TokenType::ConstantDef, pal.constant_def, false)
            } else {
                (TokenType::Identifier, pal.identifier, false)
            }
        };

        tokens.push(HighlightToken {
            text: SharedString::from(word),
            token_type,
            color,
            is_italic,
            char_count: word.chars().count() as i32,
            is_whitespace: false,
        });
    }

    HighlightLine {
        line_number,
        raw_text: String::new(),
        tokens,
    }
}

pub fn highlight_document(document: &str, theme_id: i32) -> Vec<HighlightLine> {
    if document.is_empty() {
        return vec![parse_line("", 1, theme_id)];
    }
    let line_count_est = document.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1;
    let mut raw_lines = Vec::with_capacity(line_count_est);
    raw_lines.extend(
        document
            .split('\n')
            .map(|l| l.strip_suffix('\r').unwrap_or(l)),
    );

    if raw_lines.len() > 32 {
        raw_lines
            .into_par_iter()
            .enumerate()
            .map(|(idx, line)| parse_line(line, (idx + 1) as i32, theme_id))
            .collect()
    } else {
        raw_lines
            .into_iter()
            .enumerate()
            .map(|(idx, line)| parse_line(line, (idx + 1) as i32, theme_id))
            .collect()
    }
}

pub fn build_line_starts(document: &str) -> Vec<usize> {
    let bytes = document.as_bytes();
    let mut starts = Vec::with_capacity(bytes.len() / 32 + 16);
    starts.push(0);
    for (i, &b) in bytes.iter().enumerate() {
        if b == b'\n' {
            starts.push(i + 1);
        }
    }
    starts
}

pub fn highlight_document_range_with_offsets(
    document: &str,
    line_starts: Option<&[usize]>,
    theme_id: i32,
    start_line: usize,
    count: usize,
) -> Vec<HighlightLine> {
    if document.is_empty() {
        return vec![parse_line("", 1, theme_id)];
    }

    if let Some(starts) = line_starts
        && !starts.is_empty()
    {
        if start_line >= starts.len() {
            return vec![parse_line("", (start_line + 1) as i32, theme_id)];
        }
        let start_byte = starts[start_line];
        let end_line_idx = (start_line + count).min(starts.len());
        let end_byte = if end_line_idx < starts.len() {
            starts[end_line_idx]
        } else {
            document.len()
        };

        let slice = &document[start_byte..end_byte];
        let mut lines = Vec::with_capacity(count.min(128));
        for (idx, raw_line) in slice.split('\n').enumerate() {
            if lines.len() >= count {
                break;
            }
            let clean = raw_line.strip_suffix('\r').unwrap_or(raw_line);
            lines.push(parse_line(clean, (start_line + idx + 1) as i32, theme_id));
        }
        if lines.is_empty() {
            lines.push(parse_line("", (start_line + 1) as i32, theme_id));
        }
        return lines;
    }

    let bytes = document.as_bytes();
    let mut current_line = 0;
    let mut start_byte = 0;
    while current_line < start_line && start_byte < bytes.len() {
        if bytes[start_byte] == b'\n' {
            current_line += 1;
        }
        start_byte += 1;
    }

    if start_byte >= bytes.len() && current_line < start_line {
        return vec![parse_line("", (start_line + 1) as i32, theme_id)];
    }

    let slice = &document[start_byte..];
    let mut lines = Vec::with_capacity(count.min(128));
    for (idx, raw_line) in slice.split('\n').enumerate() {
        let clean = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        lines.push(parse_line(clean, (start_line + idx + 1) as i32, theme_id));
        if lines.len() >= count {
            break;
        }
    }
    if lines.is_empty() {
        lines.push(parse_line("", (start_line + 1) as i32, theme_id));
    }
    lines
}

#[allow(dead_code)]
pub fn highlight_document_range(
    document: &str,
    theme_id: i32,
    start_line: usize,
    count: usize,
) -> Vec<HighlightLine> {
    highlight_document_range_with_offsets(document, None, theme_id, start_line, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mnemonic_recognition() {
        assert!(is_mnemonic("mov"));
        assert!(is_mnemonic("MOV"));
        assert!(is_mnemonic("lxi"));
        assert!(is_mnemonic("xra"));
        assert!(is_mnemonic("call"));
        assert!(is_mnemonic("hlt"));
        assert!(!is_mnemonic("foobar"));
    }

    #[test]
    fn test_hex_literal_detection() {
        assert!(is_hex_literal("0ffffh"));
        assert!(is_hex_literal("05h"));
        assert!(is_hex_literal("100h"));
        assert!(is_hex_literal("0x1234"));
        assert!(is_hex_literal("$FF00"));
        assert!(!is_hex_literal("hello"));
    }

    #[test]
    fn test_line_parsing_comment_and_label() {
        let line = "start:  mvi a, 05h  ; init reg A";
        let parsed = parse_line(line, 1, 0);
        assert!(!parsed.tokens.is_empty());
        assert_eq!(parsed.tokens[0].token_type, TokenType::Label);
        assert_eq!(parsed.tokens[0].text, "start");
    }

    #[test]
    fn test_constant_def_versus_label() {
        let line = "TPA equ 100h";
        let parsed = parse_line(line, 1, 1);
        assert_eq!(parsed.tokens[0].token_type, TokenType::ConstantDef);
        assert_eq!(parsed.tokens[0].text, "TPA");
        assert_eq!(parsed.tokens[2].token_type, TokenType::DirectiveDef);
        assert!(parsed.tokens[2].is_italic, "EQU directive should be italic");
    }

    #[test]
    fn test_mnemonic_directive_register_italic() {
        let doc = "org 100h\n mvi c, 09h\n lxi d, msg\n call 0005h\n ret\n end";
        let lines = highlight_document(doc, 1);
        let italic_words = ["org", "mvi", "c", "lxi", "d", "call", "ret", "end"];
        for l in lines {
            for tok in l.tokens {
                if italic_words.contains(&tok.text.as_str()) {
                    assert!(tok.is_italic, "Token {} should be italic", tok.text);
                }
            }
        }
    }

    #[test]
    fn test_highlight_document_range_with_offsets() {
        let doc = "line1: nop\nline2: hlt\nline3: mov a, b\nline4: ret\nline5: end";
        let starts = build_line_starts(doc);
        assert_eq!(starts.len(), 5);

        let slice_lines = highlight_document_range_with_offsets(doc, Some(&starts), 0, 2, 2);
        assert_eq!(slice_lines.len(), 2);
        assert_eq!(slice_lines[0].line_number, 3);
        assert_eq!(slice_lines[1].line_number, 4);

        let fallback_lines = highlight_document_range_with_offsets(doc, None, 0, 2, 2);
        assert_eq!(fallback_lines.len(), 2);
        assert_eq!(fallback_lines[0].line_number, 3);
        assert_eq!(fallback_lines[1].line_number, 4);
    }
}
