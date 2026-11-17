use crate::compiler::runner::{CompileReport, Diagnostic};
use intel_8080_kit::asm::codegen;
use intel_8080_kit::emu::{Emulator, Memory};
use intel_8080_kit::op::Opcode;
use std::cell::Cell;
use std::collections::HashMap;
use std::fs;
use std::path::Path;
use std::rc::Rc;

pub struct KitMemory {
    pub ram: Box<[u8; 65536]>,
    pub output_logs: Rc<std::cell::RefCell<Vec<String>>>,
    pub bdos_func: Cell<u8>,
    pub bdos_de_low: Cell<u8>,
    pub max_cycles: usize,
    pub cycle_count: Cell<usize>,
    pub did_timeout: Cell<bool>,
}

impl Default for KitMemory {
    fn default() -> Self {
        Self {
            ram: vec![0u8; 65536]
                .into_boxed_slice()
                .try_into()
                .unwrap_or_else(|_| Box::new([0u8; 65536])),
            output_logs: Rc::new(std::cell::RefCell::new(Vec::new())),
            bdos_func: Cell::new(0),
            bdos_de_low: Cell::new(0),
            max_cycles: 2_000_000,
            cycle_count: Cell::new(0),
            did_timeout: Cell::new(false),
        }
    }
}

impl Memory for KitMemory {
    fn read_byte(&self, addr: u16) -> u8 {
        let count = self.cycle_count.get() + 1;
        self.cycle_count.set(count);
        if self.max_cycles > 0 && count > self.max_cycles {
            self.did_timeout.set(true);
            return 0x76; // HLT opcode
        }
        self.ram[addr as usize]
    }

    fn read_word(&self, addr: u16) -> u16 {
        let low = self.read_byte(addr) as u16;
        let high = self.read_byte(addr.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    fn write_byte(&mut self, addr: u16, byte: u8) {
        self.ram[addr as usize] = byte;
    }

    fn write_word(&mut self, addr: u16, word: u16) {
        self.write_byte(addr, (word & 0xff) as u8);
        self.write_byte(addr.wrapping_add(1), (word >> 8) as u8);
    }

    fn in_port(&self, port: u8) -> u8 {
        self.output_logs
            .borrow_mut()
            .push(format!("[PORT] IN 0x{:02x}", port));
        0
    }

    fn out_port(&self, port: u8, byte: u8) {
        match port {
            0xfd => self.bdos_func.set(byte),
            0xfe => self.bdos_de_low.set(byte),
            0xff => {
                let func = self.bdos_func.get();
                let de = ((byte as u16) << 8) | (self.bdos_de_low.get() as u16);
                match func {
                    // Function 9: Print $-terminated string
                    9 => {
                        let mut bytes = Vec::new();
                        let mut addr = de;
                        while addr < 65535 {
                            let b = self.ram[addr as usize];
                            if b == b'$' {
                                break;
                            }
                            bytes.push(b);
                            addr += 1;
                        }
                        let text = String::from_utf8_lossy(&bytes).to_string();
                        for line in text.lines() {
                            self.output_logs
                                .borrow_mut()
                                .push(format!("[CPM OUTPUT] {}", line));
                        }
                    }
                    // Function 2: Console output single character
                    2 => {
                        let ch = (de & 0xff) as u8 as char;
                        self.output_logs
                            .borrow_mut()
                            .push(format!("[CPM CHAR] {}", ch));
                    }
                    // Function 0: System reset / exit
                    0 => {
                        self.output_logs
                            .borrow_mut()
                            .push("[CPM] System reset (BDOS 0)".to_string());
                    }
                    _ => {
                        self.output_logs
                            .borrow_mut()
                            .push(format!("[BDOS] Call func={}, DE=0x{:04x}", func, de));
                    }
                }
            }
            _ => {
                let ascii_repr = if byte.is_ascii_graphic() || byte == b' ' {
                    format!(" ('{}')", byte as char)
                } else {
                    String::new()
                };
                self.output_logs.borrow_mut().push(format!(
                    "[PORT 0x{:02x}] OUT 0x{:02x}{}",
                    port, byte, ascii_repr
                ));
            }
        }
    }
}

pub struct KitAssembler {
    equ_symbols: HashMap<String, u16>,
    labels: HashMap<String, u16>,
    start_org: u16,
    entry_point: Option<u16>,
}

impl KitAssembler {
    pub fn new() -> Self {
        Self {
            equ_symbols: HashMap::new(),
            labels: HashMap::new(),
            start_org: 0x0100,
            entry_point: None,
        }
    }

    pub fn assemble(&mut self, source: &str) -> Result<(Vec<u8>, usize), Vec<Diagnostic>> {
        let mut diagnostics = Vec::new();

        // Pass 1: Collect symbols, labels, and calculate layout offsets
        if let Err(errs) = self.pass1(source) {
            diagnostics.extend(errs);
            return Err(diagnostics);
        }

        // Pass 2: Generate machine code bytes
        match self.pass2(source) {
            Ok((code_binary, instruction_count)) => {
                // If start_org > 0x0100 (e.g. 0x0200 in math.asm), place JMP to entry in CP/M TPA header
                let final_binary = if self.start_org > 0x0100 {
                    let padding_len = (self.start_org - 0x0100) as usize;
                    let mut com_buf = vec![0u8; padding_len];
                    if let Some(entry) = self.entry_point {
                        // 0x0100: JMP entry (0xC3, low, high)
                        com_buf[0] = 0xc3;
                        com_buf[1] = (entry & 0xff) as u8;
                        com_buf[2] = (entry >> 8) as u8;
                    }
                    com_buf.extend_from_slice(&code_binary);
                    com_buf
                } else {
                    code_binary
                };

                Ok((final_binary, instruction_count))
            }
            Err(errs) => {
                diagnostics.extend(errs);
                Err(diagnostics)
            }
        }
    }

    fn clean_line(line: &str) -> &str {
        if let Some(pos) = line.find(';') {
            line[..pos].trim()
        } else {
            line.trim()
        }
    }

    fn pass1(&mut self, source: &str) -> Result<(), Vec<Diagnostic>> {
        let mut errors = Vec::new();
        let mut current_pc = 0x0100u16;

        for (idx, raw_line) in source.lines().enumerate() {
            let line_num = idx + 1;
            let line = Self::clean_line(raw_line);
            if line.is_empty() {
                continue;
            }

            // Check for EQU: "<NAME> equ <VALUE>" or "<NAME>: equ <VALUE>"
            let words: Vec<&str> = line.split_whitespace().collect();
            if words.len() >= 3 && words[1].eq_ignore_ascii_case("equ") {
                let name = words[0].trim_end_matches(':').to_ascii_uppercase();
                match self.parse_value(words[2]) {
                    Ok(val) => {
                        self.equ_symbols.insert(name, val);
                    }
                    Err(e) => {
                        errors.push(Diagnostic {
                            line: line_num,
                            message: format!("Invalid EQU value in '{}': {}", line, e),
                            is_error: true,
                        });
                    }
                }
                continue;
            }

            // Check for label prefix e.g. "label:" or "label: mov a, b"
            let mut stmt = line;
            if let Some(colon_pos) = line.find(':') {
                let label_name = line[..colon_pos].trim().to_ascii_uppercase();
                if !label_name.is_empty() && !label_name.contains(' ') {
                    self.labels.insert(label_name, current_pc);
                }
                stmt = line[colon_pos + 1..].trim();
            }

            if stmt.is_empty() {
                continue;
            }

            let tokens: Vec<&str> = stmt.split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }

            let mnemonic = tokens[0].to_ascii_uppercase();

            if mnemonic == "ORG" {
                if tokens.len() >= 2 {
                    match self.parse_value(tokens[1]) {
                        Ok(addr) => {
                            current_pc = addr;
                            self.start_org = addr;
                        }
                        Err(e) => {
                            errors.push(Diagnostic {
                                line: line_num,
                                message: format!("Invalid ORG address: {}", e),
                                is_error: true,
                            });
                        }
                    }
                }
                continue;
            }

            if mnemonic == "END" {
                if let Some(entry) = tokens.get(1).and_then(|t| self.parse_value(t).ok()) {
                    self.entry_point = Some(entry);
                }
                break;
            }

            if mnemonic == "DB" {
                let rest = stmt[tokens[0].len()..].trim();
                let byte_count = self.calculate_db_size(rest);
                current_pc = current_pc.wrapping_add(byte_count as u16);
                continue;
            }

            if mnemonic == "DW" {
                let rest = stmt[tokens[0].len()..].trim();
                let word_count = rest.split(',').count();
                current_pc = current_pc.wrapping_add((word_count * 2) as u16);
                continue;
            }

            if mnemonic == "DS" {
                if let Some(sz) = tokens.get(1).and_then(|t| self.parse_value(t).ok()) {
                    current_pc = current_pc.wrapping_add(sz);
                }
                continue;
            }

            // Normal instruction
            let operands = if stmt.len() > tokens[0].len() {
                stmt[tokens[0].len()..].trim()
            } else {
                ""
            };

            let size = Self::estimate_instruction_size(&mnemonic, operands);
            current_pc = current_pc.wrapping_add(size as u16);
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(errors)
        }
    }

    fn calculate_db_size(&self, rest: &str) -> usize {
        let mut count = 0;
        let mut in_quote = false;
        let mut quote_char = '\'';
        let mut escaped = false;
        let mut current_item = String::new();

        for ch in rest.chars() {
            if in_quote {
                if escaped {
                    escaped = false;
                    count += 1;
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == quote_char {
                    in_quote = false;
                } else {
                    count += 1;
                }
            } else if ch == '\'' || ch == '"' {
                in_quote = true;
                quote_char = ch;
            } else if ch == ',' {
                if !current_item.trim().is_empty() {
                    count += 1;
                    current_item.clear();
                }
            } else {
                current_item.push(ch);
            }
        }
        if !current_item.trim().is_empty() {
            count += 1;
        }
        count
    }

    fn estimate_instruction_size(mnemonic: &str, _operands: &str) -> usize {
        match mnemonic {
            "LXI" | "LDA" | "STA" | "LHLD" | "SHLD" | "JMP" | "CALL" | "JNZ" | "JZ" | "JNC"
            | "JC" | "JPO" | "JPE" | "JP" | "JM" | "CNZ" | "CZ" | "CNC" | "CC" | "CPO" | "CPE"
            | "CP" | "CM" => 3,

            "MVI" | "ADI" | "ACI" | "SUI" | "SBI" | "ANI" | "XRI" | "ORI" | "CPI" | "IN"
            | "OUT" => 2,

            _ => 1,
        }
    }

    fn parse_value(&self, s: &str) -> Result<u16, String> {
        let trimmed = s.trim().trim_end_matches(',');
        let upper = trimmed.to_ascii_uppercase();

        if let Some(&val) = self.equ_symbols.get(&upper) {
            return Ok(val);
        }
        if let Some(&addr) = self.labels.get(&upper) {
            return Ok(addr);
        }

        parse_numeric_literal(trimmed)
    }

    fn pass2(&mut self, source: &str) -> Result<(Vec<u8>, usize), Vec<Diagnostic>> {
        let mut binary = Vec::new();
        let mut errors = Vec::new();
        let mut instruction_count = 0;
        let mut current_pc = self.start_org;

        for (idx, raw_line) in source.lines().enumerate() {
            let line_num = idx + 1;
            let line = Self::clean_line(raw_line);
            if line.is_empty() {
                continue;
            }

            // Skip EQU declarations in pass 2
            let words: Vec<&str> = line.split_whitespace().collect();
            if words.len() >= 3 && words[1].eq_ignore_ascii_case("equ") {
                continue;
            }

            // Strip label prefix
            let stmt = if let Some(colon_pos) = line.find(':') {
                line[colon_pos + 1..].trim()
            } else {
                line
            };

            if stmt.is_empty() {
                continue;
            }

            let tokens: Vec<&str> = stmt.split_whitespace().collect();
            if tokens.is_empty() {
                continue;
            }

            let mnemonic = tokens[0].to_ascii_uppercase();

            if mnemonic == "ORG" || mnemonic == "EQU" {
                continue;
            }

            if mnemonic == "END" {
                if self.entry_point.is_none()
                    && let Some(ep) = tokens.get(1).and_then(|t| self.parse_value(t).ok())
                {
                    self.entry_point = Some(ep);
                }
                break;
            }

            if mnemonic == "DB" {
                let rest = stmt[tokens[0].len()..].trim();
                match self.assemble_db(rest) {
                    Ok(bytes) => {
                        current_pc = current_pc.wrapping_add(bytes.len() as u16);
                        binary.extend_from_slice(&bytes);
                    }
                    Err(e) => {
                        errors.push(Diagnostic {
                            line: line_num,
                            message: format!("DB syntax error: {}", e),
                            is_error: true,
                        });
                    }
                }
                continue;
            }

            if mnemonic == "DW" {
                let rest = stmt[tokens[0].len()..].trim();
                for item in rest.split(',') {
                    let it = item.trim();
                    if it.is_empty() {
                        continue;
                    }
                    match self.parse_value(it) {
                        Ok(word) => {
                            binary.push((word & 0xff) as u8);
                            binary.push((word >> 8) as u8);
                            current_pc = current_pc.wrapping_add(2);
                        }
                        Err(e) => {
                            errors.push(Diagnostic {
                                line: line_num,
                                message: format!("DW word error in '{}': {}", it, e),
                                is_error: true,
                            });
                        }
                    }
                }
                continue;
            }

            if mnemonic == "DS" {
                if tokens.len() >= 2 {
                    match self.parse_value(tokens[1]) {
                        Ok(sz) => {
                            binary.resize(binary.len() + sz as usize, 0);
                            current_pc = current_pc.wrapping_add(sz);
                        }
                        Err(e) => {
                            errors.push(Diagnostic {
                                line: line_num,
                                message: format!("DS error: {}", e),
                                is_error: true,
                            });
                        }
                    }
                }
                continue;
            }

            // Instruction assembly
            let operands = if stmt.len() > tokens[0].len() {
                stmt[tokens[0].len()..].trim()
            } else {
                ""
            };

            match self.assemble_instruction(&mnemonic, operands, current_pc) {
                Ok(bytes) => {
                    instruction_count += 1;
                    current_pc = current_pc.wrapping_add(bytes.len() as u16);
                    binary.extend_from_slice(&bytes);
                }
                Err(e) => {
                    errors.push(Diagnostic {
                        line: line_num,
                        message: format!("Instruction error in '{} {}': {}", mnemonic, operands, e),
                        is_error: true,
                    });
                }
            }
        }

        if errors.is_empty() {
            Ok((binary, instruction_count))
        } else {
            Err(errors)
        }
    }

    fn assemble_db(&self, rest: &str) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        let mut in_quote = false;
        let mut quote_char = '\'';
        let mut escaped = false;
        let mut current_item = String::new();

        for ch in rest.chars() {
            if in_quote {
                if escaped {
                    escaped = false;
                    match ch {
                        'n' => bytes.push(0x0a),
                        'r' => bytes.push(0x0d),
                        't' => bytes.push(0x09),
                        '0' => bytes.push(0x00),
                        '\\' => bytes.push(b'\\'),
                        '\'' => bytes.push(b'\''),
                        '"' => bytes.push(b'"'),
                        '$' => bytes.push(b'$'),
                        other => bytes.push(other as u8),
                    }
                } else if ch == '\\' {
                    escaped = true;
                } else if ch == quote_char {
                    in_quote = false;
                } else {
                    bytes.push(ch as u8);
                }
            } else if ch == '\'' || ch == '"' {
                in_quote = true;
                quote_char = ch;
            } else if ch == ',' {
                let item = current_item.trim();
                if !item.is_empty() {
                    let val = self.parse_value(item)?;
                    bytes.push((val & 0xff) as u8);
                    current_item.clear();
                }
            } else {
                current_item.push(ch);
            }
        }

        let item = current_item.trim();
        if !item.is_empty() {
            let val = self.parse_value(item)?;
            bytes.push((val & 0xff) as u8);
        }

        Ok(bytes)
    }

    fn assemble_instruction(
        &self,
        mnemonic: &str,
        operands: &str,
        _pc: u16,
    ) -> Result<Vec<u8>, String> {
        let ops = operands
            .split(',')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect::<Vec<&str>>();

        match mnemonic {
            // NOP & HLT
            "NOP" => Ok(codegen(&[Opcode::Nop])),
            "HLT" => Ok(codegen(&[Opcode::Hlt])),

            // LXI <rp>, <d16>
            "LXI" => {
                if ops.len() < 2 {
                    return Err("LXI requires 2 operands (e.g. LXI H, 0000h)".into());
                }
                let rp = ops[0].to_ascii_uppercase();
                let word = self.parse_value(ops[1])?;
                let low = (word & 0xff) as u8;
                let high = ((word >> 8) & 0xff) as u8;
                match rp.as_str() {
                    "B" => Ok(codegen(&[Opcode::LxiB(high, low)])),
                    "D" => Ok(codegen(&[Opcode::LxiD(high, low)])),
                    "H" => Ok(codegen(&[Opcode::LxiH(high, low)])),
                    "SP" => Ok(codegen(&[Opcode::LxiSp(high, low)])),
                    _ => Err(format!("Unknown register pair for LXI: {}", rp)),
                }
            }

            // MOV <r1>, <r2>
            "MOV" => {
                if ops.len() < 2 {
                    return Err("MOV requires 2 operands (e.g. MOV A, B)".into());
                }
                let r1 = ops[0].to_ascii_uppercase();
                let r2 = ops[1].to_ascii_uppercase();
                let op = match (r1.as_str(), r2.as_str()) {
                    ("A", "A") => Opcode::MovAA,
                    ("A", "B") => Opcode::MovAB,
                    ("A", "C") => Opcode::MovAC,
                    ("A", "D") => Opcode::MovAD,
                    ("A", "E") => Opcode::MovAE,
                    ("A", "H") => Opcode::MovAH,
                    ("A", "L") => Opcode::MovAL,
                    ("A", "M") => Opcode::MovAM,

                    ("B", "A") => Opcode::MovBA,
                    ("B", "B") => Opcode::MovBB,
                    ("B", "C") => Opcode::MovBC,
                    ("B", "D") => Opcode::MovBD,
                    ("B", "E") => Opcode::MovBE,
                    ("B", "H") => Opcode::MovBH,
                    ("B", "L") => Opcode::MovBL,
                    ("B", "M") => Opcode::MovBM,

                    ("C", "A") => Opcode::MovCA,
                    ("C", "B") => Opcode::MovCB,
                    ("C", "C") => Opcode::MovCC,
                    ("C", "D") => Opcode::MovCD,
                    ("C", "E") => Opcode::MovCE,
                    ("C", "H") => Opcode::MovCH,
                    ("C", "L") => Opcode::MovCL,
                    ("C", "M") => Opcode::MovCM,

                    ("D", "A") => Opcode::MovDA,
                    ("D", "B") => Opcode::MovDB,
                    ("D", "C") => Opcode::MovDC,
                    ("D", "D") => Opcode::MovDD,
                    ("D", "E") => Opcode::MovDE,
                    ("D", "H") => Opcode::MovDH,
                    ("D", "L") => Opcode::MovDL,
                    ("D", "M") => Opcode::MovDM,

                    ("E", "A") => Opcode::MovEA,
                    ("E", "B") => Opcode::MovEB,
                    ("E", "C") => Opcode::MovEC,
                    ("E", "D") => Opcode::MovED,
                    ("E", "E") => Opcode::MovEE,
                    ("E", "H") => Opcode::MovEH,
                    ("E", "L") => Opcode::MovEL,
                    ("E", "M") => Opcode::MovEM,

                    ("H", "A") => Opcode::MovHA,
                    ("H", "B") => Opcode::MovHB,
                    ("H", "C") => Opcode::MovHC,
                    ("H", "D") => Opcode::MovHD,
                    ("H", "E") => Opcode::MovHE,
                    ("H", "H") => Opcode::MovHH,
                    ("H", "L") => Opcode::MovHL,
                    ("H", "M") => Opcode::MovHM,

                    ("L", "A") => Opcode::MovLA,
                    ("L", "B") => Opcode::MovLB,
                    ("L", "C") => Opcode::MovLC,
                    ("L", "D") => Opcode::MovLD,
                    ("L", "E") => Opcode::MovLE,
                    ("L", "H") => Opcode::MovLH,
                    ("L", "L") => Opcode::MovLL,
                    ("L", "M") => Opcode::MovLM,

                    ("M", "A") => Opcode::MovMA,
                    ("M", "B") => Opcode::MovMB,
                    ("M", "C") => Opcode::MovMC,
                    ("M", "D") => Opcode::MovMD,
                    ("M", "E") => Opcode::MovME,
                    ("M", "H") => Opcode::MovMH,
                    ("M", "L") => Opcode::MovML,
                    _ => return Err(format!("Invalid registers for MOV: {}, {}", r1, r2)),
                };
                Ok(codegen(&[op]))
            }

            // MVI <r>, <d8>
            "MVI" => {
                if ops.len() < 2 {
                    return Err("MVI requires 2 operands (e.g. MVI A, 08h)".into());
                }
                let r = ops[0].to_ascii_uppercase();
                let byte = (self.parse_value(ops[1])? & 0xff) as u8;
                let op = match r.as_str() {
                    "A" => Opcode::MviA(byte),
                    "B" => Opcode::MviB(byte),
                    "C" => Opcode::MviC(byte),
                    "D" => Opcode::MviD(byte),
                    "E" => Opcode::MviE(byte),
                    "H" => Opcode::MviH(byte),
                    "L" => Opcode::MviL(byte),
                    "M" => Opcode::MviM(byte),
                    _ => return Err(format!("Unknown register for MVI: {}", r)),
                };
                Ok(codegen(&[op]))
            }

            // Memory load / store
            "LDA" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Lda(addr)]))
            }
            "STA" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Sta(addr)]))
            }
            "LHLD" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Lhld(addr)]))
            }
            "SHLD" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Shld(addr)]))
            }
            "LDAX" => {
                let rp = operands.trim().to_ascii_uppercase();
                match rp.as_str() {
                    "B" => Ok(codegen(&[Opcode::LdaxB])),
                    "D" => Ok(codegen(&[Opcode::LdaxD])),
                    _ => Err(format!("LDAX only supports B or D, found: {}", rp)),
                }
            }
            "STAX" => {
                let rp = operands.trim().to_ascii_uppercase();
                match rp.as_str() {
                    "B" => Ok(codegen(&[Opcode::StaxB])),
                    "D" => Ok(codegen(&[Opcode::StaxD])),
                    _ => Err(format!("STAX only supports B or D, found: {}", rp)),
                }
            }
            "XCHG" => Ok(codegen(&[Opcode::Xchg])),

            // DAD
            "DAD" => {
                let rp = operands.trim().to_ascii_uppercase();
                match rp.as_str() {
                    "B" => Ok(codegen(&[Opcode::DadB])),
                    "D" => Ok(codegen(&[Opcode::DadD])),
                    "H" => Ok(codegen(&[Opcode::DadH])),
                    "SP" => Ok(codegen(&[Opcode::DadSp])),
                    _ => Err(format!("Unknown register pair for DAD: {}", rp)),
                }
            }

            // Increment / Decrement
            "INR" => {
                let r = operands.trim().to_ascii_uppercase();
                let op = match r.as_str() {
                    "A" => Opcode::InrA,
                    "B" => Opcode::InrB,
                    "C" => Opcode::InrC,
                    "D" => Opcode::InrD,
                    "E" => Opcode::InrE,
                    "H" => Opcode::InrH,
                    "L" => Opcode::InrL,
                    "M" => Opcode::InrM,
                    _ => return Err(format!("Unknown register for INR: {}", r)),
                };
                Ok(codegen(&[op]))
            }
            "DCR" => {
                let r = operands.trim().to_ascii_uppercase();
                let op = match r.as_str() {
                    "A" => Opcode::DcrA,
                    "B" => Opcode::DcrB,
                    "C" => Opcode::DcrC,
                    "D" => Opcode::DcrD,
                    "E" => Opcode::DcrE,
                    "H" => Opcode::DcrH,
                    "L" => Opcode::DcrL,
                    "M" => Opcode::DcrM,
                    _ => return Err(format!("Unknown register for DCR: {}", r)),
                };
                Ok(codegen(&[op]))
            }
            "INX" => {
                let rp = operands.trim().to_ascii_uppercase();
                match rp.as_str() {
                    "B" => Ok(codegen(&[Opcode::InxB])),
                    "D" => Ok(codegen(&[Opcode::InxD])),
                    "H" => Ok(codegen(&[Opcode::InxH])),
                    "SP" => Ok(codegen(&[Opcode::InxSp])),
                    _ => Err(format!("Unknown register pair for INX: {}", rp)),
                }
            }
            "DCX" => {
                let rp = operands.trim().to_ascii_uppercase();
                match rp.as_str() {
                    "B" => Ok(codegen(&[Opcode::DcxB])),
                    "D" => Ok(codegen(&[Opcode::DcxD])),
                    "H" => Ok(codegen(&[Opcode::DcxH])),
                    "SP" => Ok(codegen(&[Opcode::DcxSp])),
                    _ => Err(format!("Unknown register pair for DCX: {}", rp)),
                }
            }

            // Arithmetic & Logic with register
            "ADD" => {
                let r = operands.trim().to_ascii_uppercase();
                let op = match r.as_str() {
                    "A" => Opcode::AddA,
                    "B" => Opcode::AddB,
                    "C" => Opcode::AddC,
                    "D" => Opcode::AddD,
                    "E" => Opcode::AddE,
                    "H" => Opcode::AddH,
                    "L" => Opcode::AddL,
                    "M" => Opcode::AddM,
                    _ => return Err(format!("Unknown register for ADD: {}", r)),
                };
                Ok(codegen(&[op]))
            }
            "ADC" => {
                let r = operands.trim().to_ascii_uppercase();
                let op = match r.as_str() {
                    "A" => Opcode::AdcA,
                    "B" => Opcode::AdcB,
                    "C" => Opcode::AdcC,
                    "D" => Opcode::AdcD,
                    "E" => Opcode::AdcE,
                    "H" => Opcode::AdcH,
                    "L" => Opcode::AdcL,
                    "M" => Opcode::AdcM,
                    _ => return Err(format!("Unknown register for ADC: {}", r)),
                };
                Ok(codegen(&[op]))
            }
            "SUB" => {
                let r = operands.trim().to_ascii_uppercase();
                let op = match r.as_str() {
                    "A" => Opcode::SubA,
                    "B" => Opcode::SubB,
                    "C" => Opcode::SubC,
                    "D" => Opcode::SubD,
                    "E" => Opcode::SubE,
                    "H" => Opcode::SubH,
                    "L" => Opcode::SubL,
                    "M" => Opcode::SubM,
                    _ => return Err(format!("Unknown register for SUB: {}", r)),
                };
                Ok(codegen(&[op]))
            }
            "SBB" => {
                let r = operands.trim().to_ascii_uppercase();
                let op = match r.as_str() {
                    "A" => Opcode::SbbA,
                    "B" => Opcode::SbbB,
                    "C" => Opcode::SbbC,
                    "D" => Opcode::SbbD,
                    "E" => Opcode::SbbE,
                    "H" => Opcode::SbbH,
                    "L" => Opcode::SbbL,
                    "M" => Opcode::SbbM,
                    _ => return Err(format!("Unknown register for SBB: {}", r)),
                };
                Ok(codegen(&[op]))
            }
            "ANA" => {
                let r = operands.trim().to_ascii_uppercase();
                let op = match r.as_str() {
                    "A" => Opcode::AnaA,
                    "B" => Opcode::AnaB,
                    "C" => Opcode::AnaC,
                    "D" => Opcode::AnaD,
                    "E" => Opcode::AnaE,
                    "H" => Opcode::AnaH,
                    "L" => Opcode::AnaL,
                    "M" => Opcode::AnaM,
                    _ => return Err(format!("Unknown register for ANA: {}", r)),
                };
                Ok(codegen(&[op]))
            }
            "XRA" => {
                let r = operands.trim().to_ascii_uppercase();
                let op = match r.as_str() {
                    "A" => Opcode::XraA,
                    "B" => Opcode::XraB,
                    "C" => Opcode::XraC,
                    "D" => Opcode::XraD,
                    "E" => Opcode::XraE,
                    "H" => Opcode::XraH,
                    "L" => Opcode::XraL,
                    "M" => Opcode::XraM,
                    _ => return Err(format!("Unknown register for XRA: {}", r)),
                };
                Ok(codegen(&[op]))
            }
            "ORA" => {
                let r = operands.trim().to_ascii_uppercase();
                let op = match r.as_str() {
                    "A" => Opcode::OraA,
                    "B" => Opcode::OraB,
                    "C" => Opcode::OraC,
                    "D" => Opcode::OraD,
                    "E" => Opcode::OraE,
                    "H" => Opcode::OraH,
                    "L" => Opcode::OraL,
                    "M" => Opcode::OraM,
                    _ => return Err(format!("Unknown register for ORA: {}", r)),
                };
                Ok(codegen(&[op]))
            }
            "CMP" => {
                let r = operands.trim().to_ascii_uppercase();
                let op = match r.as_str() {
                    "A" => Opcode::CmpA,
                    "B" => Opcode::CmpB,
                    "C" => Opcode::CmpC,
                    "D" => Opcode::CmpD,
                    "E" => Opcode::CmpE,
                    "H" => Opcode::CmpH,
                    "L" => Opcode::CmpL,
                    "M" => Opcode::CmpM,
                    _ => return Err(format!("Unknown register for CMP: {}", r)),
                };
                Ok(codegen(&[op]))
            }

            // Immediate Arithmetic & Logic
            "ADI" => {
                let b = (self.parse_value(operands)? & 0xff) as u8;
                Ok(codegen(&[Opcode::Adi(b)]))
            }
            "ACI" => {
                let b = (self.parse_value(operands)? & 0xff) as u8;
                Ok(codegen(&[Opcode::Aci(b)]))
            }
            "SUI" => {
                let b = (self.parse_value(operands)? & 0xff) as u8;
                Ok(codegen(&[Opcode::Sui(b)]))
            }
            "SBI" => {
                let b = (self.parse_value(operands)? & 0xff) as u8;
                Ok(codegen(&[Opcode::Sbi(b)]))
            }
            "ANI" => {
                let b = (self.parse_value(operands)? & 0xff) as u8;
                Ok(codegen(&[Opcode::Ani(b)]))
            }
            "XRI" => {
                let b = (self.parse_value(operands)? & 0xff) as u8;
                Ok(codegen(&[Opcode::Xri(b)]))
            }
            "ORI" => {
                let b = (self.parse_value(operands)? & 0xff) as u8;
                Ok(codegen(&[Opcode::Ori(b)]))
            }
            "CPI" => {
                let b = (self.parse_value(operands)? & 0xff) as u8;
                Ok(codegen(&[Opcode::Cpi(b)]))
            }

            // Rotates & Flags
            "RLC" => Ok(codegen(&[Opcode::Rlc])),
            "RRC" => Ok(codegen(&[Opcode::Rrc])),
            "RAL" => Ok(codegen(&[Opcode::Ral])),
            "RAR" => Ok(codegen(&[Opcode::Rar])),
            "CMA" => Ok(codegen(&[Opcode::Cma])),
            "CMC" => Ok(codegen(&[Opcode::Cmc])),
            "STC" => Ok(codegen(&[Opcode::Stc])),
            "DAA" => Ok(codegen(&[Opcode::Daa])),
            "EI" => Ok(codegen(&[Opcode::Ei])),
            "DI" => Ok(codegen(&[Opcode::Di])),

            // Jumps
            "JMP" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Jmp(addr)]))
            }
            "JNZ" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Jnz(addr)]))
            }
            "JZ" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Jz(addr)]))
            }
            "JNC" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Jnc(addr)]))
            }
            "JC" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Jc(addr)]))
            }
            "JPO" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Jpo(addr)]))
            }
            "JPE" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Jpe(addr)]))
            }
            "JP" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Jp(addr)]))
            }
            "JM" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Jm(addr)]))
            }

            // Calls
            "CALL" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Call(addr)]))
            }
            "CNZ" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Cnz(addr)]))
            }
            "CZ" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Cz(addr)]))
            }
            "CNC" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Cnc(addr)]))
            }
            "CC" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Cc(addr)]))
            }
            "CPO" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Cpo(addr)]))
            }
            "CPE" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Cpe(addr)]))
            }
            "CP" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Cp(addr)]))
            }
            "CM" => {
                let addr = self.parse_value(operands)?;
                Ok(codegen(&[Opcode::Cm(addr)]))
            }

            // Returns
            "RET" => Ok(codegen(&[Opcode::Ret])),
            "RNZ" => Ok(codegen(&[Opcode::Rnz])),
            "RZ" => Ok(codegen(&[Opcode::Rz])),
            "RNC" => Ok(codegen(&[Opcode::Rnc])),
            "RC" => Ok(codegen(&[Opcode::Rc])),
            "RPO" => Ok(codegen(&[Opcode::Rpo])),
            "RPE" => Ok(codegen(&[Opcode::Rpe])),
            "RP" => Ok(codegen(&[Opcode::Rp])),
            "RM" => Ok(codegen(&[Opcode::Rm])),

            // Stack
            "PUSH" => {
                let rp = operands.trim().to_ascii_uppercase();
                match rp.as_str() {
                    "B" => Ok(codegen(&[Opcode::PushB])),
                    "D" => Ok(codegen(&[Opcode::PushD])),
                    "H" => Ok(codegen(&[Opcode::PushH])),
                    "PSW" => Ok(codegen(&[Opcode::PushPsw])),
                    _ => Err(format!("Unknown operand for PUSH: {}", rp)),
                }
            }
            "POP" => {
                let rp = operands.trim().to_ascii_uppercase();
                match rp.as_str() {
                    "B" => Ok(codegen(&[Opcode::PopB])),
                    "D" => Ok(codegen(&[Opcode::PopD])),
                    "H" => Ok(codegen(&[Opcode::PopH])),
                    "PSW" => Ok(codegen(&[Opcode::PopPsw])),
                    _ => Err(format!("Unknown operand for POP: {}", rp)),
                }
            }
            "XTHL" => Ok(codegen(&[Opcode::Xthl])),
            "SPHL" => Ok(codegen(&[Opcode::Sphl])),
            "PCHL" => Ok(codegen(&[Opcode::Pchl])),

            // I/O
            "IN" => {
                let p = (self.parse_value(operands)? & 0xff) as u8;
                Ok(codegen(&[Opcode::In(p)]))
            }
            "OUT" => {
                let p = (self.parse_value(operands)? & 0xff) as u8;
                Ok(codegen(&[Opcode::Out(p)]))
            }

            _ => Err(format!("Unsupported or unrecognized opcode: {}", mnemonic)),
        }
    }
}

fn parse_numeric_literal(s: &str) -> Result<u16, String> {
    let s = s.trim();
    if s.is_empty() {
        return Err("Empty numeric string".into());
    }

    // Trailing 'h' or 'H' hex e.g. 100h, 0005h, 0ffffh
    if (s.ends_with('h') || s.ends_with('H')) && s.len() > 1 {
        let hex = &s[..s.len() - 1];
        if hex.chars().all(|c| c.is_ascii_hexdigit()) {
            return u16::from_str_radix(hex, 16)
                .map_err(|e| format!("Failed to parse hex '{}': {}", s, e));
        }
    }

    // Trailing 'b' or 'B' binary e.g. 10101010b
    if (s.ends_with('b') || s.ends_with('B')) && s.len() > 1 {
        let bin = &s[..s.len() - 1];
        if bin.chars().all(|c| c == '0' || c == '1') {
            return u16::from_str_radix(bin, 2)
                .map_err(|e| format!("Failed to parse binary '{}': {}", s, e));
        }
    }

    // Trailing 'o' / 'q' octal
    if (s.ends_with('o') || s.ends_with('O') || s.ends_with('q') || s.ends_with('Q')) && s.len() > 1
    {
        let oct = &s[..s.len() - 1];
        if oct.chars().all(|c| ('0'..='7').contains(&c)) {
            return u16::from_str_radix(oct, 8)
                .map_err(|e| format!("Failed to parse octal '{}': {}", s, e));
        }
    }

    // Hex prefix 0x / 0X
    if let Some(h) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
        return u16::from_str_radix(h, 16)
            .map_err(|e| format!("Failed to parse hex '{}': {}", s, e));
    }

    // Hex prefix $
    if let Some(h) = s.strip_prefix('$') {
        return u16::from_str_radix(h, 16)
            .map_err(|e| format!("Failed to parse hex '{}': {}", s, e));
    }

    // Decimal
    s.parse::<u16>()
        .map_err(|e| format!("Failed to parse number '{}': {}", s, e))
}

pub fn assemble_file(file_path: &Path) -> CompileReport {
    let mut logs = Vec::new();
    let com_path = file_path.with_extension("com");

    logs.push(format!(
        "[{}] [ASSEMBLER:intel-8080-kit] Compiling: {}",
        get_timestamp(),
        file_path.display()
    ));

    let content = match fs::read_to_string(file_path) {
        Ok(s) => s,
        Err(e) => {
            logs.push(format!(
                "[{}] [ERROR] Failed to read source file: {}",
                get_timestamp(),
                e
            ));
            return CompileReport {
                success: false,
                output_logs: logs,
                diagnostics: vec![Diagnostic {
                    line: 1,
                    message: format!("Failed to read source file: {}", e),
                    is_error: true,
                }],
                instruction_count: 0,
                estimated_bytes: 0,
                com_path: None,
            };
        }
    };

    let mut assembler = KitAssembler::new();
    match assembler.assemble(&content) {
        Ok((bin, count)) => {
            if let Err(e) = fs::write(&com_path, &bin) {
                logs.push(format!(
                    "[{}] [ERROR] Failed to write binary '{}': {}",
                    get_timestamp(),
                    com_path.display(),
                    e
                ));
                return CompileReport {
                    success: false,
                    output_logs: logs,
                    diagnostics: vec![Diagnostic {
                        line: 1,
                        message: format!("Failed to write COM file: {}", e),
                        is_error: true,
                    }],
                    instruction_count: count,
                    estimated_bytes: bin.len(),
                    com_path: None,
                };
            }

            logs.push(format!(
                "[{}] [SUCCESS:intel-8080-kit] Assembled {} instructions ({} bytes) -> {}",
                get_timestamp(),
                count,
                bin.len(),
                com_path.file_name().unwrap_or_default().to_string_lossy()
            ));

            CompileReport {
                success: true,
                output_logs: logs,
                diagnostics: Vec::new(),
                instruction_count: count,
                estimated_bytes: bin.len(),
                com_path: Some(com_path),
            }
        }
        Err(errs) => {
            for err in &errs {
                logs.push(format!(
                    "[{}] [ERROR] Line {}: {}",
                    get_timestamp(),
                    err.line,
                    err.message
                ));
            }
            CompileReport {
                success: false,
                output_logs: logs,
                diagnostics: errs,
                instruction_count: 0,
                estimated_bytes: 0,
                com_path: None,
            }
        }
    }
}

pub fn run_com_file(com_path: &Path) -> Vec<String> {
    let mut logs = Vec::new();

    logs.push(format!(
        "[{}] [EMULATOR:intel-8080-kit] Loading binary: {}",
        get_timestamp(),
        com_path.display()
    ));

    let bin_data = match fs::read(com_path) {
        Ok(b) => b,
        Err(e) => {
            logs.push(format!(
                "[{}] [ERROR] Failed to read COM file: {}",
                get_timestamp(),
                e
            ));
            return logs;
        }
    };

    let output_logs = Rc::new(std::cell::RefCell::new(Vec::new()));
    let mut mem = Box::new(KitMemory {
        output_logs: output_logs.clone(),
        ..Default::default()
    });

    // 0x0000: Bootloader stub (JMP 0x0040)
    mem.write_byte(0x0000, 0xc3);
    mem.write_byte(0x0001, 0x40);
    mem.write_byte(0x0002, 0x00);

    // 0x0005: CP/M BDOS Interception Hook (10 bytes)
    // 0x79:       MOV A, C
    // 0xD3, 0xFD: OUT 0xFD, A
    // 0x7B:       MOV A, E
    // 0xD3, 0xFE: OUT 0xFE, A
    // 0x7A:       MOV A, D
    // 0xD3, 0xFF: OUT 0xFF, A
    // 0xC9:       RET
    let bdos_hook: [u8; 10] = [0x79, 0xd3, 0xfd, 0x7b, 0xd3, 0xfe, 0x7a, 0xd3, 0xff, 0xc9];
    for (i, &b) in bdos_hook.iter().enumerate() {
        mem.write_byte(0x0005 + i as u16, b);
    }

    // 0x0040: Bootloader setup
    // LXI SP, 0xF000 (0x31, 0x00, 0xF0)
    // CALL 0x0100    (0xCD, 0x00, 0x01)
    // HLT            (0x76)
    mem.write_byte(0x0040, 0x31);
    mem.write_byte(0x0041, 0x00);
    mem.write_byte(0x0042, 0xf0);
    mem.write_byte(0x0043, 0xcd);
    mem.write_byte(0x0044, 0x00);
    mem.write_byte(0x0045, 0x01);
    mem.write_byte(0x0046, 0x76);

    // Load COM program bytes at 0x0100 (standard CP/M TPA)
    for (i, &b) in bin_data.iter().enumerate() {
        let addr = 0x0100 + i;
        if addr < 65536 {
            mem.write_byte(addr as u16, b);
        }
    }

    // Safety sentinels: Place HLT right after program and at 0xFFFF to prevent runaway execution
    let end_addr = 0x0100 + bin_data.len();
    if end_addr < 65536 {
        mem.write_byte(end_addr as u16, 0x76);
    }
    mem.write_byte(0xffff, 0x76);

    let mut emu = Emulator::new(mem);
    emu.run();

    // Collect captured program output logs
    let captured = output_logs.borrow().clone();
    logs.extend(captured);

    logs.push(format!(
        "[{}] [EMULATOR:intel-8080-kit] Execution completed.",
        get_timestamp()
    ));

    logs
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
    fn test_assemble_8080_cpm_string() {
        let src = r#"
TPA     equ 100h
BDOS    equ 0005h
WSTRF   equ 09h

        org TPA
        
        mvi c, WSTRF
        lxi d, msg
        call BDOS
        
        ret

msg:    db 'Hello, World!\n$'

        end
        "#;

        let mut assembler = KitAssembler::new();
        let (bin, count) = assembler.assemble(src).expect("Assembly failed");
        println!(
            "Compiled 8080.asm ({} bytes, {} instructions): {:02x?}",
            bin.len(),
            count,
            bin
        );
        assert_eq!(bin.len(), 24);
        assert_eq!(count, 4);

        let temp_com = std::env::temp_dir().join("test_8080_cpm.com");
        fs::write(&temp_com, &bin).expect("Failed to write temp com");

        let logs = run_com_file(&temp_com);
        println!("Run logs:\n{}", logs.join("\n"));
        let _ = fs::remove_file(&temp_com);

        assert!(logs.iter().any(|l| l.contains("Hello, World!")));
    }

    #[test]
    fn test_assemble_math_routines() {
        let src = r#"
        org     0200h

multiply_8:
        lxi     h, 0000h
        mvi     a, 08h
        mov     d, a

mult_loop:
        dad     h
        mov     a, b
        ral
        mov     b, a
        jnc     no_add

        push    d
        mvi     d, 00h
        mov     e, c
        dad     d
        pop     d

no_add:
        dcr     d
        jnz     mult_loop
        ret

start_math:
        mvi     b, 07h
        mvi     c, 06h
        call    multiply_8
        hlt

        end     start_math
        "#;

        let mut assembler = KitAssembler::new();
        let (bin, count) = assembler.assemble(src).expect("Assembly failed");
        println!(
            "Compiled math.asm ({} bytes, {} instructions)",
            bin.len(),
            count
        );
        assert!(bin.len() > 0);

        let temp_com = std::env::temp_dir().join("test_math.com");
        fs::write(&temp_com, &bin).expect("Failed to write temp com");

        let logs = run_com_file(&temp_com);
        println!("Run logs:\n{}", logs.join("\n"));
        let _ = fs::remove_file(&temp_com);

        assert!(logs.iter().any(|l| l.contains("Execution completed")));
    }

    #[test]
    fn test_assemble_main_routine() {
        let src = r#"
        org     0100h
start:
        lxi     sp, 0ffffh
        lxi     h, array_data
        mvi     c, 05h
        xra     a

sum_loop:
        mov     b, m
        add     b
        inx     h
        dcr     c
        jnz     sum_loop

        sta     result

        call    print_done
        hlt

print_done:
        mvi     a, 0ffh
        sta     status_flag
        ret

status_flag:    db      00h
result:         db      00h
array_data:     db      0ah, 14h, 1eh, 28h, 32h

        end     start
        "#;

        let mut assembler = KitAssembler::new();
        let (bin, count) = assembler.assemble(src).expect("Assembly failed");
        println!(
            "Compiled main.asm ({} bytes, {} instructions)",
            bin.len(),
            count
        );
        assert!(bin.len() > 0);

        let temp_com = std::env::temp_dir().join("test_main.com");
        fs::write(&temp_com, &bin).expect("Failed to write temp com");

        let logs = run_com_file(&temp_com);
        println!("Run logs:\n{}", logs.join("\n"));
        let _ = fs::remove_file(&temp_com);

        assert!(logs.iter().any(|l| l.contains("Execution completed")));
    }
}
