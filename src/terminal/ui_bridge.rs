use super::emulator::TerminalEmulator;
use super::pty::{PtyEvent, PtyHost};
use crate::ui::AppWindow;
use slint::{ComponentHandle, SharedString};
use slint_term::core::{Button, Modifiers, MouseAction};
use slint_term::{Action, Shortcut, TerminalBinding, Theme, install_window_input};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::mpsc::Receiver;

pub struct TerminalUiBridge {
    _timer: slint::Timer,
}

impl TerminalUiBridge {
    pub fn setup(app: &AppWindow, initial_cwd: PathBuf) -> Self {
        let emulator = Arc::new(TerminalEmulator::new(80, 24));
        let cwd_cell = Rc::new(RefCell::new(initial_cwd));

        let pty_slot = Rc::new(RefCell::new(None::<PtyHost>));
        let rx_slot = Rc::new(RefCell::new(None::<Receiver<PtyEvent>>));

        // Helper to launch or restart the PTY session
        let spawn_pty = {
            let cwd_cell = Rc::clone(&cwd_cell);
            let pty_slot = Rc::clone(&pty_slot);
            let rx_slot = Rc::clone(&rx_slot);
            let app_weak = app.as_weak();

            move || {
                let cwd = cwd_cell.borrow().clone();
                match PtyHost::spawn(&cwd, 80, 24) {
                    Ok((pty, rx)) => {
                        let shell_name = pty.shell_name().to_string();
                        *pty_slot.borrow_mut() = Some(pty);
                        *rx_slot.borrow_mut() = Some(rx);

                        if let Some(ui) = app_weak.upgrade() {
                            ui.set_term_shell_name(SharedString::from(&shell_name));
                            ui.set_term_status_text(SharedString::from(format!(
                                "{shell_name} (Работает)"
                            )));
                        }
                    }
                    Err(e) => {
                        if let Some(ui) = app_weak.upgrade() {
                            ui.set_term_status_text(SharedString::from(format!("Ошибка: {e}")));
                        }
                    }
                }
            }
        };

        // Initial spawn
        spawn_pty();

        // Create the TerminalBinding
        let pty_sink_slot = Rc::clone(&pty_slot);
        let binding = Rc::new(RefCell::new(TerminalBinding::attach(
            emulator.inner(),
            move |action| match action {
                Action::Write(bytes) => {
                    if let Some(pty) = pty_sink_slot.borrow().as_ref() {
                        pty.write(bytes);
                    }
                }
                Action::Resize(sz) => {
                    if let Some(pty) = pty_sink_slot.borrow().as_ref() {
                        pty.resize(sz.cols(), sz.rows());
                    }
                }
                Action::Shortcut(Shortcut::Clear) => {
                    // Handled internally in binding
                }
                Action::Shortcut(_) => {}
                Action::Error(err) => {
                    eprintln!("[Terminal Error] {err}");
                }
                _ => {}
            },
        )));

        // Configure font and default theme
        {
            let mut b = binding.borrow_mut();
            b.set_theme(Theme::fstty("dark", "default"));
            b.set_font("CaskaydiaCove Nerd Font Mono", 14.0);
        }

        // Install OS-level window keyboard/IME handler for the terminal
        {
            let binding_for_input = Rc::clone(&binding);
            let app_weak = app.as_weak();
            install_window_input(app.window(), move || {
                let ui = app_weak.upgrade()?;
                // Active ONLY when:
                // 1. Terminal tab is selected in bottom panel (tab == 1)
                // 2. Terminal panel is expanded
                // 3. Terminal currently has keyboard focus
                // 4. No modal dialog is open (settings, new file/folder, rename)
                if ui.get_terminal_active_tab() == 1
                    && ui.get_terminal_expanded()
                    && ui.get_term_focused()
                    && !ui.get_show_settings_dialog()
                    && !ui.get_show_new_file_dialog()
                    && !ui.get_show_new_folder_dialog()
                    && !ui.get_show_rename_dialog()
                {
                    Some(binding_for_input.clone())
                } else {
                    None
                }
            });
        }

        // Pointer / click event callback
        {
            let binding_ptr = Rc::clone(&binding);
            let app_weak = app.as_weak();
            app.on_term_pointer(move |x, y, kind, btn, ctrl, alt, shift| {
                if kind == 0 {
                    if let Some(ui) = app_weak.upgrade() {
                        ui.set_term_focused(true);
                    }
                    if let Ok(mut b) = binding_ptr.try_borrow_mut() {
                        b.focus(true);
                    }
                }
                let action = match kind {
                    0 => MouseAction::Press,
                    1 => MouseAction::Release,
                    _ => return,
                };
                let button = match btn {
                    0 => Button::Left,
                    1 => Button::Right,
                    2 => Button::Middle,
                    _ => Button::None,
                };
                let modifiers = Modifiers { ctrl, alt, shift };
                if let Ok(mut b) = binding_ptr.try_borrow_mut() {
                    b.pointer(x, y, action, button, modifiers);
                }
            });
        }

        // Motion callback
        {
            let binding_mot = Rc::clone(&binding);
            app.on_term_motion(move |x, y| {
                if let Ok(mut b) = binding_mot.try_borrow_mut() {
                    b.motion(x, y);
                }
            });
        }

        // Mouse wheel callback
        {
            let binding_whl = Rc::clone(&binding);
            app.on_term_wheel(move |x, y, delta, ctrl, alt, shift| {
                let modifiers = Modifiers { ctrl, alt, shift };
                if let Ok(mut b) = binding_whl.try_borrow_mut() {
                    b.wheel(x, y, delta, modifiers);
                }
            });
        }

        // Scrollbar scroll callback
        {
            let binding_scr = Rc::clone(&binding);
            app.on_term_scroll_to(move |frac| {
                if let Ok(mut b) = binding_scr.try_borrow_mut() {
                    b.scroll_to(frac);
                }
            });
        }

        // Focus changed callback
        {
            let binding_foc = Rc::clone(&binding);
            app.on_term_focus_changed(move |has_focus| {
                if let Ok(mut b) = binding_foc.try_borrow_mut() {
                    b.focus(has_focus);
                }
            });
        }

        // Terminal Restart callback
        {
            let spawn_fn = spawn_pty.clone();
            let emulator_rst = Arc::clone(&emulator);
            let binding_rst = Rc::clone(&binding);
            let app_weak_rst = app.as_weak();

            app.on_term_restart(move || {
                emulator_rst.clear();
                if let Some(ui) = app_weak_rst.upgrade() {
                    ui.set_term_focused(true);
                }
                if let Ok(mut b) = binding_rst.try_borrow_mut() {
                    b.action(Shortcut::Clear);
                    b.focus(true);
                }
                spawn_fn();
            });
        }

        // Terminal Clear callback
        {
            let emulator_clr = Arc::clone(&emulator);
            let binding_clr = Rc::clone(&binding);
            let pty_clr = Rc::clone(&pty_slot);

            app.on_term_clear(move || {
                emulator_clr.clear();
                if let Ok(mut b) = binding_clr.try_borrow_mut() {
                    b.action(Shortcut::Clear);
                }
                // Send clear sequence to PTY shell
                if let Some(pty) = pty_clr.borrow().as_ref() {
                    pty.write(b"\x1b[2J\x1b[H".to_vec());
                }
            });
        }

        // Send command directly to PTY terminal callback
        {
            let pty_send = Rc::clone(&pty_slot);
            let binding_send = Rc::clone(&binding);
            let app_weak_send = app.as_weak();
            app.on_term_send_command(move |cmd| {
                if let Some(pty) = pty_send.borrow().as_ref() {
                    let mut bytes = cmd.as_bytes().to_vec();
                    bytes.extend_from_slice(b"\r");
                    pty.write(bytes);
                }
                if let Some(ui) = app_weak_send.upgrade() {
                    ui.set_term_focused(true);
                }
                if let Ok(mut b) = binding_send.try_borrow_mut() {
                    b.focus(true);
                }
            });
        }

        // Rate-limited UI update timer (60 FPS / ~16ms)
        let timer = slint::Timer::default();
        let timer_binding = Rc::clone(&binding);
        let timer_app = app.as_weak();
        let timer_rx = Rc::clone(&rx_slot);
        let timer_pty = Rc::clone(&pty_slot);
        let timer_emulator = Arc::clone(&emulator);

        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(16),
            move || {
                let Some(ui) = timer_app.upgrade() else { return; };

                // 1. Drain incoming PTY byte chunks with rate limit budget
                if let Some(rx) = timer_rx.borrow().as_ref() {
                    let mut bytes_budget: usize = 256 * 1024;
                    while let Ok(event) = rx.try_recv() {
                        match event {
                            PtyEvent::Output(chunk) => {
                                let len = chunk.len();
                                timer_emulator.feed(&chunk);
                                if bytes_budget > len {
                                    bytes_budget -= len;
                                } else {
                                    break;
                                }
                            }
                            PtyEvent::Exited(code) => {
                                let shell_name = timer_pty
                                    .borrow()
                                    .as_ref()
                                    .map(|p| p.shell_name().to_string())
                                    .unwrap_or_else(|| "Shell".into());
                                let status_text = match code {
                                    Some(c) => format!("{shell_name} (Выход {c})"),
                                    None => format!("{shell_name} (Завершен)"),
                                };
                                ui.set_term_status_text(SharedString::from(status_text));
                            }
                            PtyEvent::Error(err) => {
                                ui.set_term_status_text(SharedString::from(format!("Ошибка: {err}")));
                            }
                        }
                    }

                    // Forward emulator response events (e.g. DSR cursor position report, device attributes, window title)
                    for event in timer_emulator.drain_events() {
                        match event {
                            slint_term::core::TerminalEvent::Write(bytes) => {
                                if let Some(pty) = timer_pty.borrow().as_ref() {
                                    pty.write(bytes);
                                }
                            }
                            slint_term::core::TerminalEvent::Title(title) if !title.is_empty() => {
                                ui.set_term_status_text(SharedString::from(format!("{title} (Работает)")));
                            }
                            _ => {}
                        }
                    }
                }

                // 2. Synchronize layout size with terminal view
                let Ok(mut b) = timer_binding.try_borrow_mut() else { return; };
                let w = ui.get_term_view_width();
                let h = ui.get_term_view_height();
                let scale = ui.window().scale_factor();
                if w > 20.0 && h > 20.0 {
                    b.fit(w, h, scale);
                }

                // 3. Render updated rows if dirty
                let refreshed = b.refresh();
                if refreshed > 0 {
                    ui.set_term_bands(b.model());
                    let (cw, ch) = b.cell_size();
                    ui.set_term_cell_width(cw);
                    ui.set_term_line_height(ch);
                    let (h_lines, h_offset) = b.history();
                    ui.set_term_history_lines(h_lines as i32);
                    ui.set_term_history_offset(h_offset as i32);
                    let (c_col, c_row) = b.cursor_position();
                    ui.set_term_cursor_col(c_col);
                    ui.set_term_cursor_row(c_row);
                    ui.set_term_preedit(SharedString::from(&b.preedit));
                }
            },
        );

        Self { _timer: timer }
    }
}
