use intel_8080_kit::asm::{codegen, lexer::tokenize};
use intel_8080_kit::emu::{Emulator, Memory};
use std::cell::{Cell, RefCell};
use std::rc::Rc;

struct TestMemory {
    ram: [u8; 65536],
    output: Rc<RefCell<Vec<String>>>,
    bdos_func: Cell<u8>,
    bdos_de_low: Cell<u8>,
}

impl Memory for TestMemory {
    fn read_byte(&self, addr: u16) -> u8 {
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
        self.output
            .borrow_mut()
            .push(format!("IN port 0x{:02x}", port));
        0
    }

    fn out_port(&self, port: u8, byte: u8) {
        match port {
            0xfd => self.bdos_func.set(byte),
            0xfe => self.bdos_de_low.set(byte),
            0xff => {
                let func = self.bdos_func.get();
                let de = ((byte as u16) << 8) | (self.bdos_de_low.get() as u16);
                if func == 9 {
                    let mut s = Vec::new();
                    let mut addr = de;
                    while addr < 65535 {
                        let b = self.ram[addr as usize];
                        if b == b'$' {
                            break;
                        }
                        s.push(b);
                        addr += 1;
                    }
                    let text = String::from_utf8_lossy(&s).to_string();
                    self.output.borrow_mut().push(text);
                } else if func == 2 {
                    let ch = self.bdos_de_low.get() as char;
                    self.output.borrow_mut().push(ch.to_string());
                }
            }
            _ => {
                self.output
                    .borrow_mut()
                    .push(format!("OUT port 0x{:02x}: 0x{:02x}", port, byte));
            }
        }
    }
}

#[allow(dead_code)]
fn preprocess_asm(src: &str) -> (String, u16) {
    let mut out_lines = Vec::new();
    let mut start_org = 0x0100u16;
    let mut equ_map = std::collections::HashMap::new();

    // Pass 1: Collect EQU definitions and strip comments
    for line in src.lines() {
        let code = if let Some(idx) = line.find(';') {
            &line[..idx]
        } else {
            line
        }
        .trim();

        if code.is_empty() {
            continue;
        }

        let parts: Vec<&str> = code.split_whitespace().collect();
        if parts.len() >= 3 && parts[1].eq_ignore_ascii_case("equ") {
            let name = parts[0].to_ascii_uppercase();
            let val_str = normalize_num_str(parts[2]);
            if let Ok(val) = parse_u16(&val_str) {
                equ_map.insert(name, val);
            }
        }
    }

    // Pass 2: Process lines, transform LXI and numbers, filter directives
    for line in src.lines() {
        let code = if let Some(idx) = line.find(';') {
            &line[..idx]
        } else {
            line
        }
        .trim();

        if code.is_empty() {
            continue;
        }

        let mut tokens: Vec<String> = Vec::new();
        for t in code.split_whitespace() {
            let mut tok = t.to_string();
            // Replace known EQU constants
            let upper = tok.trim_matches(',').to_ascii_uppercase();
            if let Some(&val) = equ_map.get(&upper) {
                let comma = if tok.ends_with(',') { "," } else { "" };
                tok = format!("0x{:x}{}", val, comma);
            }
            tokens.push(tok);
        }

        if tokens.is_empty() {
            continue;
        }

        let op = tokens[0].to_ascii_lowercase();
        if op == "equ" || (tokens.len() >= 2 && tokens[1].eq_ignore_ascii_case("equ")) {
            continue;
        }

        if op == "end" {
            continue;
        }

        if op == "org" {
            if tokens.len() >= 2 {
                let clean_num = normalize_num_str(&tokens[1]);
                if let Ok(addr) = parse_u16(&clean_num) {
                    start_org = addr;
                }
            }
            continue;
        }

        // Transform LXI: lxi <reg>, <16-bit word> -> lxi <reg> <low_byte> <high_byte>
        if op == "lxi" && tokens.len() >= 3 {
            let reg = tokens[1].trim_matches(',').to_ascii_lowercase();
            let operand = normalize_num_str(tokens[2].trim_matches(','));
            if let Ok(word) = parse_u16(&operand) {
                let low = (word & 0xFF) as u8;
                let high = ((word >> 8) & 0xFF) as u8;
                out_lines.push(format!("lxi {} 0x{:02x} 0x{:02x}", reg, low, high));
                continue;
            }
        }

        // General instruction line: normalize trailing h hex numbers
        let mut line_buf = String::new();
        for (i, tok) in tokens.iter().enumerate() {
            if i > 0 {
                line_buf.push(' ');
            }
            let is_label = tok.ends_with(':');
            let has_comma = tok.ends_with(',');
            let clean = tok.trim_end_matches(',');
            if !is_label {
                let norm = normalize_num_str(clean);
                line_buf.push_str(&norm);
                if has_comma {
                    line_buf.push(',');
                }
            } else {
                line_buf.push_str(tok);
            }
        }
        out_lines.push(line_buf);
    }

    (out_lines.join("\n"), start_org)
}

#[allow(dead_code)]
fn normalize_num_str(s: &str) -> String {
    let trimmed = s.trim();
    if (trimmed.ends_with('h') || trimmed.ends_with('H')) && trimmed.len() > 1 {
        let hex_part = &trimmed[..trimmed.len() - 1];
        if hex_part.chars().all(|c| c.is_ascii_hexdigit()) {
            return format!("0x{}", hex_part);
        }
    }
    trimmed.to_string()
}

#[allow(dead_code)]
fn parse_u16(s: &str) -> Result<u16, ()> {
    let s = s.trim();
    if let Some(h) = s.strip_prefix("0x") {
        u16::from_str_radix(h, 16).map_err(|_| ())
    } else if let Some(h) = s.strip_prefix("0X") {
        u16::from_str_radix(h, 16).map_err(|_| ())
    } else {
        s.parse::<u16>().map_err(|_| ())
    }
}

#[test]
fn test_assemble_math_routine() {
    let src = "
    start:
        mvi b, 0x07
        mvi c, 0x06
        call multiply_8
        hlt

    multiply_8:
        lxi h 0x00 0x00
        mvi a, 0x08
        mov d, a
    mult_loop:
        dad h
        mov a, b
        ral
        mov b, a
        jnc no_add
        push d
        mvi d, 0x00
        mov e, c
        dad d
        pop d
    no_add:
        dcr d
        jnz mult_loop
        ret
    ";

    let mut ops = vec![intel_8080_kit::op::Opcode::LxiSp(0xff, 0xf0)];
    let mut parsed_ops = tokenize(src).expect("Tokenize failed on math routine");
    // Fix jump/call offsets because we prepended LxiSp (3 bytes)
    for op in &mut parsed_ops {
        match op {
            intel_8080_kit::op::Opcode::Call(addr)
            | intel_8080_kit::op::Opcode::Jnz(addr)
            | intel_8080_kit::op::Opcode::Jnc(addr) => {
                *addr += 3;
            }
            _ => {}
        }
    }
    ops.append(&mut parsed_ops);

    let bin = codegen(&ops);
    println!("Compiled math routine ({} bytes): {:02x?}", bin.len(), bin);

    let output = Rc::new(RefCell::new(Vec::new()));
    let mut mem = Box::new(TestMemory {
        ram: [0; 65536],
        output: output.clone(),
        bdos_func: Cell::new(0),
        bdos_de_low: Cell::new(0),
    });
    for (i, &b) in bin.iter().enumerate() {
        mem.write_byte(i as u16, b);
    }

    let mut emu = Emulator::new(mem);
    emu.run();
    println!("Math routine ran to HLT successfully with intel-8080-kit!");
}

#[test]
fn test_assemble_with_jump_and_label() {
    let asm = "
        mvi c, 5
    loop:
        dcr c
        jnz loop
        hlt
    ";
    let ops = tokenize(asm).expect("Failed to tokenize loop");
    let bin = codegen(&ops);
    println!("Loop machine code: {:02x?}", bin);
    let output = Rc::new(RefCell::new(Vec::new()));
    let mut mem = Box::new(TestMemory {
        ram: [0; 65536],
        output: output.clone(),
        bdos_func: Cell::new(0),
        bdos_de_low: Cell::new(0),
    });
    for (i, &b) in bin.iter().enumerate() {
        mem.write_byte(i as u16, b);
    }

    let mut emu = Emulator::new(mem);
    emu.run();
    println!("Loop executed successfully!");
}

#[test]
fn test_cpm_bdos_print_string() {
    let output = Rc::new(RefCell::new(Vec::new()));
    let mut mem = Box::new(TestMemory {
        ram: [0; 65536],
        output: output.clone(),
        bdos_func: Cell::new(0),
        bdos_de_low: Cell::new(0),
    });

    // 0x0000: Bootloader jump to 0x0040
    mem.write_byte(0x0000, 0xc3); // JMP 0x0040
    mem.write_byte(0x0001, 0x40);
    mem.write_byte(0x0002, 0x00);

    // 0x0040: Bootloader setup stack and call program
    mem.write_byte(0x0040, 0x31); // LXI SP, 0xF000
    mem.write_byte(0x0041, 0x00);
    mem.write_byte(0x0042, 0xf0);
    mem.write_byte(0x0043, 0xcd); // CALL 0x0100
    mem.write_byte(0x0044, 0x00);
    mem.write_byte(0x0045, 0x01);
    mem.write_byte(0x0046, 0x76); // HLT

    // 0x0005: BDOS Hook (10 bytes)
    let bdos_bytes: [u8; 10] = [
        0x79, // MOV A, C
        0xd3, 0xfd, // OUT 0xFD, A
        0x7b, // MOV A, E
        0xd3, 0xfe, // OUT 0xFE, A
        0x7a, // MOV A, D
        0xd3, 0xff, // OUT 0xFF, A
        0xc9, // RET
    ];
    for (i, &b) in bdos_bytes.iter().enumerate() {
        mem.write_byte(0x0005 + i as u16, b);
    }

    // 0x0100: User Program (MVI C, 9; LXI D, 0x010A; CALL 0x0005; RET)
    let prog_bytes: [u8; 9] = [
        0x0e, 0x09, // MVI C, 9
        0x11, 0x09, 0x01, // LXI D, 0x0109
        0xcd, 0x05, 0x00, // CALL 0x0005
        0xc9, // RET
    ];
    for (i, &b) in prog_bytes.iter().enumerate() {
        mem.write_byte(0x0100 + i as u16, b);
    }

    // String at 0x0109: "Hello, World!\n$"
    let msg = b"Hello, World!\n$";
    for (i, &b) in msg.iter().enumerate() {
        mem.write_byte(0x0109 + i as u16, b);
    }

    let mut emu = Emulator::new(mem);
    emu.run();

    let logs = output.borrow().clone();
    println!("Emulator output logs: {:?}", logs);
    assert_eq!(logs.len(), 1);
    assert_eq!(logs[0], "Hello, World!\n");
    println!("BDOS Print String test passed successfully!");
}
