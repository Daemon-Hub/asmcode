use portable_pty::{CommandBuilder, MasterPty, NativePtySystem, PtySize, PtySystem};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;

#[derive(Debug)]
pub enum PtyCommand {
    Write(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    Kill,
}

#[derive(Debug)]
pub enum PtyEvent {
    Output(Vec<u8>),
    Exited(Option<u32>),
    Error(String),
}

pub struct PtyHost {
    cmd_tx: Sender<PtyCommand>,
    #[allow(dead_code)]
    master: Arc<Mutex<Box<dyn MasterPty + Send>>>,
    shell_name: String,
}

#[allow(dead_code)]
impl PtyHost {
    pub fn spawn(cwd: &Path, cols: u16, rows: u16) -> Result<(Self, Receiver<PtyEvent>), String> {
        let pty_system = NativePtySystem::default();
        let pair = pty_system
            .openpty(PtySize {
                rows: rows.max(4),
                cols: cols.max(10),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("Failed to open PTY: {e}"))?;

        let candidates = detect_shell_candidates();
        let mut spawned_child = None;
        let mut chosen_name = String::from("shell");

        for (cmd_path, display_name) in candidates {
            let mut cmd = CommandBuilder::new(&cmd_path);
            if cwd.exists() && cwd.is_dir() {
                cmd.cwd(cwd);
            }
            if display_name == "pwsh" || display_name == "powershell" {
                cmd.args(["-NoLogo"]);
            }

            match pair.slave.spawn_command(cmd) {
                Ok(child) => {
                    spawned_child = Some(child);
                    chosen_name = display_name;
                    break;
                }
                Err(_) => continue,
            }
        }

        let mut child = match spawned_child {
            Some(c) => c,
            None => {
                return Err("Failed to spawn any suitable shell (pwsh, cmd, powershell)".into());
            }
        };

        // Close slave in host process so EOF is triggered when the child process exits
        drop(pair.slave);

        let mut reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("Failed to clone PTY reader: {e}"))?;
        let mut writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("Failed to take PTY writer: {e}"))?;

        let master = Arc::new(Mutex::new(pair.master));
        let (event_tx, event_rx) = channel::<PtyEvent>();
        let (cmd_tx, cmd_rx) = channel::<PtyCommand>();

        // Background reader thread: continuously reads raw bytes from master PTY
        let reader_event_tx = event_tx.clone();
        thread::Builder::new()
            .name("pty-reader".into())
            .spawn(move || {
                let mut buf = [0u8; 8192];
                loop {
                    match reader.read(&mut buf) {
                        Ok(0) => {
                            let _ = reader_event_tx.send(PtyEvent::Exited(None));
                            break;
                        }
                        Ok(n) => {
                            if reader_event_tx.send(PtyEvent::Output(buf[..n].to_vec())).is_err() {
                                break;
                            }
                        }
                        Err(e) => {
                            let _ = reader_event_tx.send(PtyEvent::Error(e.to_string()));
                            let _ = reader_event_tx.send(PtyEvent::Exited(None));
                            break;
                        }
                    }
                }
            })
            .map_err(|e| format!("Failed to spawn PTY reader thread: {e}"))?;

        // Background command and writer thread
        let master_for_cmds = Arc::clone(&master);
        thread::Builder::new()
            .name("pty-writer".into())
            .spawn(move || {
                while let Ok(cmd) = cmd_rx.recv() {
                    match cmd {
                        PtyCommand::Write(bytes) => {
                            if writer.write_all(&bytes).is_err() || writer.flush().is_err() {
                                break;
                            }
                        }
                        PtyCommand::Resize { cols, rows } => {
                            if let Ok(m) = master_for_cmds.lock() {
                                let _ = m.resize(PtySize {
                                    rows: rows.max(1),
                                    cols: cols.max(2),
                                    pixel_width: 0,
                                    pixel_height: 0,
                                });
                            }
                        }
                        PtyCommand::Kill => {
                            let _ = child.kill();
                            break;
                        }
                    }
                }
                let _ = child.kill();
                let _ = child.wait();
            })
            .map_err(|e| format!("Failed to spawn PTY writer thread: {e}"))?;

        Ok((
            Self {
                cmd_tx,
                master,
                shell_name: chosen_name,
            },
            event_rx,
        ))
    }

    pub fn command_sender(&self) -> Sender<PtyCommand> {
        self.cmd_tx.clone()
    }

    pub fn shell_name(&self) -> &str {
        &self.shell_name
    }

    pub fn write(&self, data: Vec<u8>) {
        let _ = self.cmd_tx.send(PtyCommand::Write(data));
    }

    pub fn resize(&self, cols: u16, rows: u16) {
        let _ = self.cmd_tx.send(PtyCommand::Resize { cols, rows });
    }

    pub fn kill(&self) {
        let _ = self.cmd_tx.send(PtyCommand::Kill);
    }
}

impl Drop for PtyHost {
    fn drop(&mut self) {
        let _ = self.cmd_tx.send(PtyCommand::Kill);
    }
}

#[cfg(windows)]
fn detect_shell_candidates() -> Vec<(String, String)> {
    let mut candidates = Vec::new();
    candidates.push(("pwsh.exe".into(), "pwsh".into()));
    if let Ok(comspec) = std::env::var("COMSPEC")
        && !comspec.is_empty()
    {
        candidates.push((comspec, "cmd".into()));
    }
    candidates.push(("cmd.exe".into(), "cmd".into()));
    candidates.push(("powershell.exe".into(), "powershell".into()));
    candidates
}

#[cfg(not(windows))]
fn detect_shell_candidates() -> Vec<(String, String)> {
    let sh = std::env::var("SHELL").unwrap_or_else(|_| "sh".into());
    let name = Path::new(&sh)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("sh")
        .to_string();
    vec![(sh, name)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pty_spawn_and_lifecycle() {
        let (pty, rx) = PtyHost::spawn(Path::new("."), 80, 24).expect("Failed to spawn PTY");
        let emu = crate::terminal::TerminalEmulator::new(80, 24);

        let mut got_output = false;
        let start = std::time::Instant::now();
        while start.elapsed() < std::time::Duration::from_secs(4) {
            if let Ok(event) = rx.recv_timeout(std::time::Duration::from_millis(50)) {
                if let PtyEvent::Output(bytes) = event {
                    emu.feed(&bytes);
                    for ev in emu.drain_events() {
                        if let slint_term::core::TerminalEvent::Write(reply) = ev {
                            pty.write(reply);
                        }
                    }
                    got_output = true;
                    if bytes.len() > 20 {
                        break;
                    }
                }
            }
        }
        assert!(got_output, "Expected at least some output from PTY shell");
        pty.kill();
    }
}
