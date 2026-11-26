use intel_8080_kit::emu::{Emulator, Memory};
use std::cell::{Cell, RefCell};
use std::fs;
use std::path::Path;
use std::rc::Rc;

pub struct KitMemory {
    pub ram: Box<[u8; 65536]>,
    pub output_logs: Rc<RefCell<Vec<String>>>,
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
            output_logs: Rc::new(RefCell::new(Vec::new())),
            bdos_func: Cell::new(0),
            bdos_de_low: Cell::new(0),
            max_cycles: 2_000_000,
            cycle_count: Cell::new(0),
            did_timeout: Cell::new(false),
        }
    }
}

impl Memory for KitMemory {
    #[inline(always)]
    fn read_byte(&self, addr: u16) -> u8 {
        let count = self.cycle_count.get() + 1;
        self.cycle_count.set(count);
        if self.max_cycles > 0 && count > self.max_cycles {
            if !self.did_timeout.get() {
                self.did_timeout.set(true);
                self.output_logs.borrow_mut().push(
                    "[WARN] Execution aborted: cycle limit exceeded (possible infinite loop)."
                        .to_string(),
                );
            }
            return 0x76; // HLT opcode
        }
        self.ram[addr as usize]
    }

    #[inline(always)]
    fn read_word(&self, addr: u16) -> u16 {
        let low = self.read_byte(addr) as u16;
        let high = self.read_byte(addr.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    #[inline(always)]
    fn write_byte(&mut self, addr: u16, byte: u8) {
        self.ram[addr as usize] = byte;
    }

    #[inline(always)]
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
                        let text = String::from_utf8_lossy(&bytes);
                        let mut logs = self.output_logs.borrow_mut();
                        for line in text.lines() {
                            logs.push(format!("[CPM OUTPUT] {}", line));
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
                    // Function 1: Console input (character)
                    1 => {
                        self.output_logs.borrow_mut().push(
                            "[WARN] Встроенный эмулятор intel-8080-kit не поддерживает интерактивный ввод с клавиатуры (BDOS 01h). Переключитесь на внешний cpmemu в Настройках."
                                .to_string(),
                        );
                    }
                    // Function 10: Read console buffer
                    10 => {
                        self.output_logs.borrow_mut().push(
                            "[WARN] Встроенный эмулятор intel-8080-kit не поддерживает интерактивный ввод с клавиатуры (BDOS 0Ah). Переключитесь на внешний cpmemu в Настройках."
                                .to_string(),
                        );
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

pub(crate) fn get_timestamp() -> String {
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

    let output_logs = Rc::new(RefCell::new(Vec::new()));
    let mut mem = Box::new(KitMemory {
        output_logs: output_logs.clone(),
        ..Default::default()
    });

    // 0x0000: Bootloader stub (JMP 0x0040)
    mem.write_byte(0x0000, 0xc3);
    mem.write_byte(0x0001, 0x40);
    mem.write_byte(0x0002, 0x00);

    // 0x0005: CP/M BDOS Interception Hook (10 bytes)
    let bdos_hook: [u8; 10] = [0x79, 0xd3, 0xfd, 0x7b, 0xd3, 0xfe, 0x7a, 0xd3, 0xff, 0xc9];
    for (i, &b) in bdos_hook.iter().enumerate() {
        mem.write_byte(0x0005 + i as u16, b);
    }

    // 0x0040: Bootloader setup (LXI SP, 0xF000; CALL 0x0100; HLT)
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

    let captured = output_logs.borrow();
    logs.extend(captured.iter().cloned());
    drop(captured);

    logs.push(format!(
        "[{}] [EMULATOR:intel-8080-kit] Execution completed.",
        get_timestamp()
    ));

    logs
}
