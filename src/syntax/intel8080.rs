use slint::Color;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum TokenType {
    Mnemonic = 0,
    Directive = 1,
    Register = 2,
    Label = 3,
    NumberHex = 4,
    NumberDec = 5,
    StringLiteral = 6,
    Comment = 7,
    Punctuation = 8,
    Text = 9,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct HighlightToken {
    pub text: String,
    pub token_type: TokenType,
    pub color: Color,
}

#[allow(dead_code)]
#[derive(Clone, Debug)]
pub struct HighlightLine {
    pub line_number: i32,
    pub raw_text: String,
    pub tokens: Vec<HighlightToken>,
}

const COLOR_MNEMONIC: Color = Color::from_rgb_u8(97, 175, 239); // #61afef cyan-blue
const COLOR_DIRECTIVE: Color = Color::from_rgb_u8(198, 120, 221); // #c678dd purple
const COLOR_REGISTER: Color = Color::from_rgb_u8(209, 154, 102); // #d19a66 peach
const COLOR_LABEL: Color = Color::from_rgb_u8(229, 192, 123); // #e5c07b amber gold
const COLOR_NUMBER_HEX: Color = Color::from_rgb_u8(152, 195, 121); // #98c379 mint green
const COLOR_NUMBER_DEC: Color = Color::from_rgb_u8(152, 195, 121); // #98c379 mint green
const COLOR_STRING: Color = Color::from_rgb_u8(224, 108, 117); // #e06c75 coral red
const COLOR_COMMENT: Color = Color::from_rgb_u8(127, 132, 142); // #7f848e slate gray
const COLOR_PUNCT: Color = Color::from_rgb_u8(171, 178, 191); // #abb2bf light gray
const COLOR_TEXT: Color = Color::from_rgb_u8(205, 214, 244); // #cdd6f4 white text

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

static INSTRUCTION_TABLE: [u32; 78] = [
    pack4(b"CC\0\0"),
    pack4(b"JC\0\0"),
    pack4(b"RC\0\0"),
    pack4(b"DI\0\0"),
    pack4(b"EI\0\0"),
    pack4(b"CM\0\0"),
    pack4(b"JM\0\0"),
    pack4(b"RM\0\0"),
    pack4(b"IN\0\0"),
    pack4(b"CP\0\0"),
    pack4(b"JP\0\0"),
    pack4(b"RP\0\0"),
    pack4(b"CZ\0\0"),
    pack4(b"JZ\0\0"),
    pack4(b"RZ\0\0"),
    pack4(b"DAA\0"),
    pack4(b"LDA\0"),
    pack4(b"CMA\0"),
    pack4(b"ANA\0"),
    pack4(b"ORA\0"),
    pack4(b"XRA\0"),
    pack4(b"STA\0"),
    pack4(b"SBB\0"),
    pack4(b"SUB\0"),
    pack4(b"ADC\0"),
    pack4(b"RLC\0"),
    pack4(b"CMC\0"),
    pack4(b"CNC\0"),
    pack4(b"JNC\0"),
    pack4(b"RNC\0"),
    pack4(b"RRC\0"),
    pack4(b"STC\0"),
    pack4(b"DAD\0"),
    pack4(b"ADD\0"),
    pack4(b"CPE\0"),
    pack4(b"JPE\0"),
    pack4(b"RPE\0"),
    pack4(b"SBI\0"),
    pack4(b"ACI\0"),
    pack4(b"ADI\0"),
    pack4(b"ANI\0"),
    pack4(b"CPI\0"),
    pack4(b"ORI\0"),
    pack4(b"XRI\0"),
    pack4(b"SUI\0"),
    pack4(b"MVI\0"),
    pack4(b"LXI\0"),
    pack4(b"RAL\0"),
    pack4(b"CPO\0"),
    pack4(b"JPO\0"),
    pack4(b"RPO\0"),
    pack4(b"CMP\0"),
    pack4(b"JMP\0"),
    pack4(b"NOP\0"),
    pack4(b"POP\0"),
    pack4(b"RAR\0"),
    pack4(b"DCR\0"),
    pack4(b"INR\0"),
    pack4(b"RET\0"),
    pack4(b"HLT\0"),
    pack4(b"RST\0"),
    pack4(b"OUT\0"),
    pack4(b"MOV\0"),
    pack4(b"DCX\0"),
    pack4(b"INX\0"),
    pack4(b"CNZ\0"),
    pack4(b"JNZ\0"),
    pack4(b"RNZ\0"),
    pack4(b"LHLD"),
    pack4(b"SHLD"),
    pack4(b"XCHG"),
    pack4(b"PUSH"),
    pack4(b"PCHL"),
    pack4(b"SPHL"),
    pack4(b"XTHL"),
    pack4(b"CALL"),
    pack4(b"LDAX"),
    pack4(b"STAX"),
];

static DIRECTIVE_TABLE: [u32; 7] = [
    pack4(b"DB\0\0"),
    pack4(b"DS\0\0"),
    pack4(b"DW\0\0"),
    pack4(b"END\0"),
    pack4(b"ORG\0"),
    pack4(b"SET\0"),
    pack4(b"EQU\0"),
];

#[inline(always)]
pub fn is_mnemonic(word: &str) -> bool {
    let bytes = word.as_bytes();
    let len = bytes.len();
    if !(2..=4).contains(&len) {
        return false;
    }
    let packed = pack_mnemonic_u32(bytes);
    INSTRUCTION_TABLE.binary_search(&packed).is_ok()
}

#[inline(always)]
pub fn is_directive(word: &str) -> bool {
    let bytes = word.as_bytes();
    let len = bytes.len();
    if !(2..=4).contains(&len) {
        return false;
    }
    let packed = pack_mnemonic_u32(bytes);
    DIRECTIVE_TABLE.binary_search(&packed).is_ok()
}

#[inline(always)]
pub fn is_register(word: &str) -> bool {
    let bytes = word.as_bytes();
    if bytes.len() == 1 {
        let b = bytes[0] | 0x20;
        matches!(b, b'a' | b'b' | b'c' | b'd' | b'e' | b'h' | b'l' | b'm')
    } else if bytes.len() == 2 {
        let packed = pack_mnemonic_u32(bytes);
        packed == pack4(b"SP\0\0")
    } else if bytes.len() == 3 {
        let packed = pack_mnemonic_u32(bytes);
        packed == pack4(b"PSW\0")
    } else {
        false
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

    let last = bytes[len - 1];
    if (last == b'h' || last == b'H') && len > 1 {
        let mut i = 0;
        while i < len - 1 {
            if (CHAR_MAP[bytes[i] as usize] & CC_HEX) == 0 {
                return false;
            }
            i += 1;
        }
        return true;
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
    while i < bytes.len() {
        if (CHAR_MAP[bytes[i] as usize] & CC_DEC) == 0 {
            return false;
        }
        i += 1;
    }
    true
}

#[inline]
pub fn parse_line(line: &str, line_number: i32) -> HighlightLine {
    let bytes = line.as_bytes();
    let len = bytes.len();
    let mut tokens = Vec::with_capacity(8);
    let mut i = 0;

    while i < len {
        let b = bytes[i];

        if b == b';' {
            tokens.push(HighlightToken {
                text: line[i..].to_string(),
                token_type: TokenType::Comment,
                color: COLOR_COMMENT,
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
            tokens.push(HighlightToken {
                text: line[start..i].to_string(),
                token_type: TokenType::StringLiteral,
                color: COLOR_STRING,
            });
            continue;
        }

        if (CHAR_MAP[b as usize] & CC_SPACE) != 0 {
            let start = i;
            while i < len && (CHAR_MAP[bytes[i] as usize] & CC_SPACE) != 0 {
                i += 1;
            }
            tokens.push(HighlightToken {
                text: line[start..i].to_string(),
                token_type: TokenType::Text,
                color: COLOR_TEXT,
            });
            continue;
        }

        if (CHAR_MAP[b as usize] & CC_PUNCT) != 0 {
            tokens.push(HighlightToken {
                text: line[i..=i].to_string(),
                token_type: TokenType::Punctuation,
                color: COLOR_PUNCT,
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
                text: word.to_string(),
                token_type: TokenType::Label,
                color: COLOR_LABEL,
            });
            tokens.push(HighlightToken {
                text: ":".to_string(),
                token_type: TokenType::Punctuation,
                color: COLOR_PUNCT,
            });
            i += 1;
            continue;
        }

        let (token_type, color) = if is_mnemonic(word) {
            (TokenType::Mnemonic, COLOR_MNEMONIC)
        } else if is_directive(word) {
            (TokenType::Directive, COLOR_DIRECTIVE)
        } else if is_register(word) {
            (TokenType::Register, COLOR_REGISTER)
        } else if is_hex_literal(word) {
            (TokenType::NumberHex, COLOR_NUMBER_HEX)
        } else if is_dec_literal(word) {
            (TokenType::NumberDec, COLOR_NUMBER_DEC)
        } else {
            let mut j = i;
            while j < len && (CHAR_MAP[bytes[j] as usize] & CC_SPACE) != 0 {
                j += 1;
            }
            if j + 3 <= len && line[j..].to_ascii_uppercase().starts_with("EQU") {
                (TokenType::Label, COLOR_LABEL)
            } else {
                (TokenType::Text, COLOR_TEXT)
            }
        };

        tokens.push(HighlightToken {
            text: word.to_string(),
            token_type,
            color,
        });
    }

    HighlightLine {
        line_number,
        raw_text: line.to_string(),
        tokens,
    }
}

pub fn highlight_document(document: &str) -> Vec<HighlightLine> {
    let line_count = document.lines().count();
    let mut lines = Vec::with_capacity(line_count.min(1000));
    // For large documents over 800 lines, limit highlighting to first 800 lines to avoid UI thread freeze
    let limit = 800;
    for (idx, line) in document.lines().take(limit).enumerate() {
        lines.push(parse_line(line, (idx + 1) as i32));
    }
    lines
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
        let parsed = parse_line(line, 1);
        assert!(!parsed.tokens.is_empty());
        assert_eq!(parsed.tokens[0].token_type, TokenType::Label);
        assert_eq!(parsed.tokens[0].text, "start");
    }
}
