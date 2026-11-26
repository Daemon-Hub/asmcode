use crate::ui::AppWindow;
use crate::ui::controller::{IdeController, persist_current_session};
use crate::ui::editor_bridge::{ensure_active_file_or_create_draft, update_highlight_and_labels};
use crate::ui::explorer_bridge::refresh_files_ui;
use crate::ui::handlers::explorer_ops::attach_explorer_handlers;
use crate::ui::handlers::settings_ops::attach_settings_handlers;
use crate::ui::handlers::tab_ops::attach_tab_handlers;
pub use crate::ui::handlers::tab_ops::sync_tabs_ui;
use crate::ui::terminal::sync_logs;
use slint::{Color, ComponentHandle, SharedString};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

pub fn open_file_internal(
    ui: &AppWindow,
    ctrl_rc: &Arc<Mutex<IdeController>>,
    path: &PathBuf,
    is_force_load: bool,
) -> bool {
    let existing_tab = {
        let ctrl = ctrl_rc.lock().unwrap();
        if let Some(exact) = ctrl.open_tabs.iter().find(|open_p| *open_p == path) {
            Some(exact.clone())
        } else {
            let canonical_target = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
            ctrl.open_tabs
                .iter()
                .find(|open_p| {
                    std::fs::canonicalize(open_p).unwrap_or_else(|_| (*open_p).clone())
                        == canonical_target
                })
                .cloned()
        }
    };

    let target_path_buf = if let Some(ref matched_path) = existing_tab {
        let is_already_active = {
            let ctrl = ctrl_rc.lock().unwrap();
            ctrl.file_manager.active_file_path.as_ref() == Some(matched_path)
        };
        if is_already_active {
            return true;
        }
        {
            let mut ctrl = ctrl_rc.lock().unwrap();
            if let Some(curr) = ctrl.file_manager.active_file_path.clone() {
                let is_windowed = ctrl
                    .large_window
                    .as_ref()
                    .is_some_and(|w| w.path == curr);
                if ui.get_is_dirty() {
                    ctrl.dirty_tabs.insert(curr.clone());
                    if !is_windowed {
                        let current_code = ui.get_code_text().to_string();
                        ctrl.set_tab_buffer(curr, current_code);
                    }
                } else {
                    ctrl.dirty_tabs.remove(&curr);
                    if !is_windowed && !ctrl.tab_buffers.contains_key(&curr) {
                        let current_code = ui.get_code_text().to_string();
                        ctrl.set_tab_buffer(curr, current_code);
                    }
                }
            }
        }
        matched_path.clone()
    } else {
        path.clone()
    };
    let path = &target_path_buf;

    let (is_draft, has_cached) = {
        let ctrl = ctrl_rc.lock().unwrap();
        let is_draft = ctrl.untitled_drafts.contains(path) || !path.exists();
        let has_cached = ctrl.tab_buffers.contains_key(path);
        (is_draft, has_cached)
    };

    if !path.exists() && !is_draft {
        return false;
    }

    let file_size = if is_draft {
        let ctrl = ctrl_rc.lock().unwrap();
        if !ctrl.tab_buffers.contains_key(path) {
            drop(ctrl);
            let mut ctrl = ctrl_rc.lock().unwrap();
            ctrl.set_tab_buffer(path.clone(), String::new());
            0
        } else {
            ctrl.tab_buffers.get(path).map_or(0, |s| s.len() as u64)
        }
    } else {
        let metadata = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(err) => {
                let mut ctrl = ctrl_rc.lock().unwrap();
                ctrl.append_log(&format!(
                    "[ERROR] Could not read metadata for '{}': {}",
                    path.display(),
                    err
                ));
                sync_logs(ui, &ctrl);
                return false;
            }
        };

        if !has_cached {
            match crate::fs_manager::explorer::is_binary_file(path) {
                Ok(true) => {
                    let file_name = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "file".to_string());
                    let is_ru = ui.get_current_language() == "ru";
                    let modal_msg = if is_ru {
                        format!(
                            "Файл \"{}\" является бинарным или имеет неподдерживаемый формат.\nОткрытие таких файлов в текстовом редакторе отключено.",
                            file_name
                        )
                    } else {
                        format!(
                            "File \"{}\" appears to be binary or uses an unsupported format.\nOpening binary files in the text editor is not supported.",
                            file_name
                        )
                    };
                    ui.set_binary_file_modal_text(SharedString::from(modal_msg));
                    ui.set_show_binary_file_modal(true);
                    ui.set_status_message(SharedString::from(if is_ru {
                        "Бинарный файл не поддерживается"
                    } else {
                        "Binary file not supported"
                    }));
                    ui.set_status_color(Color::from_rgb_u8(249, 226, 175));
                    let mut ctrl = ctrl_rc.lock().unwrap();
                    ctrl.append_log(&format!("[WARN] Skipped binary file '{}'", path.display()));
                    sync_logs(ui, &ctrl);
                    return false;
                }
                Ok(false) => {}
                Err(err) => {
                    let mut ctrl = ctrl_rc.lock().unwrap();
                    ctrl.append_log(&format!(
                        "[ERROR] Could not inspect file '{}': {}",
                        path.display(),
                        err
                    ));
                    sync_logs(ui, &ctrl);
                    return false;
                }
            }
        }

        let file_size = metadata.len();
        let (large_opt_enabled, hard_limit_bytes) = {
            let ctrl = ctrl_rc.lock().unwrap();
            (
                ctrl.session.large_file_optimizations,
                ctrl.session.large_file_hard_limit_mb * 1024 * 1024,
            )
        };

        if !has_cached && large_opt_enabled && file_size > hard_limit_bytes && !is_force_load {
            let file_name = path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".to_string());
            let size_mb = file_size / (1024 * 1024);
            let msg = if ui.get_current_language() == "ru" {
                format!(
                    "Файл '{}' имеет размер {} МБ (> 500 МБ). Загрузка в память может замедлить работу системы. Вы уверены, что хотите открыть его?",
                    file_name, size_mb
                )
            } else {
                format!(
                    "File '{}' is {} MB (> 500 MB). Loading into RAM may cause performance drops. Are you sure you want to open it?",
                    file_name, size_mb
                )
            };
            ui.set_pending_large_file_path(SharedString::from(path.to_string_lossy().to_string()));
            ui.set_large_file_modal_text(SharedString::from(msg));
            ui.set_show_large_file_modal(true);
            return false;
        }

        if !has_cached {
            let mut ctrl = ctrl_rc.lock().unwrap();
            let content = match ctrl.file_manager.read_file_content(path) {
                Ok(c) => c,
                Err(err) => {
                    ctrl.append_log(&format!(
                        "[ERROR] Could not read file '{}': {}",
                        path.display(),
                        err
                    ));
                    sync_logs(ui, &ctrl);
                    return false;
                }
            };
            ctrl.set_tab_buffer(path.clone(), content);
        }
        file_size
    };

    let mut ctrl = ctrl_rc.lock().unwrap();
    let (byte_len, total_lines) = {
        let len = ctrl.tab_buffers.get(path).map_or(0, String::len);
        let lines = ctrl
            .tab_line_starts
            .get(path)
            .map_or(1, |s| s.len().max(1));
        (len, lines)
    };

    let (large_opt_enabled, soft_limit_bytes) = (
        ctrl.session.large_file_optimizations,
        ctrl.session.large_file_soft_limit_mb * 1024 * 1024,
    );

    let is_asm = path
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| {
            e.eq_ignore_ascii_case("asm")
                || e.eq_ignore_ascii_case("inc")
                || e.eq_ignore_ascii_case("s")
                || e.eq_ignore_ascii_case("mac")
        });

    let is_large_file = is_asm
        && large_opt_enabled
        && (file_size > soft_limit_bytes
            || total_lines > ctrl.session.large_file_soft_limit_lines);
    let use_window = crate::ui::controller::should_use_windowed_text(byte_len, total_lines);

    ctrl.is_large_file_mode = is_large_file;
    ctrl.force_full_features = false;
    ctrl.viewport_cache = None;
    ui.set_is_large_file_mode(is_large_file);
    ui.set_is_active_asm(is_asm);
    crate::ui::editor_bridge::reset_editor_ui_caches();

    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "NewFile.asm".to_string());

    ui.set_active_file_name(SharedString::from(&file_name));
    ui.set_active_file_path(SharedString::from(path.to_string_lossy().to_string()));
    let is_dirty = ctrl.dirty_tabs.contains(path);
    ui.set_is_dirty(is_dirty);

    if use_window {
        let (win_text, win_state, _) = {
            let buf = ctrl.tab_buffers.get(path).map(String::as_str).unwrap_or("");
            let starts = ctrl
                .tab_line_starts
                .get(path)
                .map(Vec::as_slice)
                .unwrap_or(&[0]);
            crate::ui::controller::build_window_slice_for_path(path.clone(), buf, starts, 0)
        };
        ui.set_is_windowed_large_file(true);
        ui.set_window_read_only(win_state.read_only);
        ui.set_total_line_count(total_lines as i32);
        ui.set_text_window_start_line(win_state.start_line as i32);
        ui.set_code_text(SharedString::from(win_text));
        ctrl.large_window = Some(win_state);
        ctrl.undo_manager.disable();
    } else {
        ctrl.large_window = None;
        ui.set_is_windowed_large_file(false);
        ui.set_window_read_only(false);
        ui.set_total_line_count(total_lines as i32);
        ui.set_text_window_start_line(0);
        let cloned_for_undo = if byte_len <= crate::ui::controller::LARGE_WINDOW_BYTES_THRESHOLD {
            ctrl.tab_buffers.get(path).cloned()
        } else {
            None
        };
        {
            let buf = ctrl.tab_buffers.get(path).map(String::as_str).unwrap_or("");
            ui.set_code_text(SharedString::from(buf));
        }
        if let Some(undo_buf) = cloned_for_undo {
            ctrl.undo_manager.reset(undo_buf, 0);
        } else {
            ctrl.undo_manager.disable();
        }
    }

    ui.set_scroll_to_line_request(0);
    ui.set_scroll_to_line_tick(ui.get_scroll_to_line_tick().wrapping_add(1));

    if !ctrl.open_tabs.contains(path) {
        ctrl.open_tabs.push(path.clone());
    }
    ctrl.file_manager.active_file_path = Some(path.clone());

    ctrl.append_log(&format!(
        "[FILE] Opened '{}' ({} bytes, {} mode)",
        path.display(),
        file_size,
        if use_window {
            "windowed large file"
        } else if is_large_file {
            "large file"
        } else {
            "standard"
        }
    ));
    sync_logs(ui, &ctrl);
    sync_tabs_ui(ui, &ctrl);
    persist_current_session(ui, &ctrl);

    let labels = {
        let buf = ctrl.tab_buffers.get(path).map(String::as_str).unwrap_or("");
        let line_starts = ctrl.tab_line_starts.get(path).map(Vec::as_slice);
        crate::ui::editor_bridge::update_highlight_and_labels_opts(
            ui,
            &ctrl.autocomplete,
            buf,
            line_starts,
            is_asm,
            true,
        )
    };
    ctrl.cached_symbols_path = Some(path.clone());
    ctrl.cached_user_symbols = std::sync::Arc::new(labels);
    true
}

pub fn attach_file_handlers(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
    attach_tab_handlers(app, controller_rc);
    attach_explorer_handlers(app, controller_rc);
    attach_settings_handlers(app, controller_rc);

    // Large file modal confirmation
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_confirm_open_large_file(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let path = PathBuf::from(path_str.as_str());
        open_file_internal(&ui, &ctrl_clone, &path, true);
    });

    app.on_cancel_open_large_file(move || {});

    // Force enable features in large file mode
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_force_enable_large_file_features(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        ui.set_is_large_file_mode(false);
        let mut ctrl = ctrl_clone.lock().unwrap();
        ctrl.is_large_file_mode = false;
        ctrl.force_full_features = true;
        ctrl.invalidate_viewport_cache();
        crate::ui::editor_bridge::reset_editor_ui_caches();
        let is_asm = ui.get_is_active_asm();
        ctrl.append_log("[SYSTEM] Large file mode forcibly disabled by user");
        sync_logs(&ui, &ctrl);
        if let Some(active_path) = ctrl.file_manager.active_file_path.clone()
            && let Some(buf) = ctrl.tab_buffers.get(&active_path)
        {
            let line_starts = ctrl.tab_line_starts.get(&active_path).map(Vec::as_slice);
            let labels = crate::ui::editor_bridge::update_highlight_and_labels_opts(
                &ui,
                &ctrl.autocomplete,
                buf.as_str(),
                line_starts,
                is_asm,
                true,
            );
            ctrl.cached_symbols_path = Some(active_path);
            ctrl.cached_user_symbols = std::sync::Arc::new(labels);
        } else {
            let text = ui.get_code_text().to_string();
            let auto = ctrl.autocomplete.clone();
            drop(ctrl);
            update_highlight_and_labels(&ui, &auto, &text, is_asm);
        }
    });

    // Save File
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_save_file(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        ensure_active_file_or_create_draft(&ui, &mut ctrl);

        let active_name = ui.get_active_file_name().to_string();
        if active_name.is_empty() {
            return;
        }
        let save_path = if let Some(path) = &ctrl.file_manager.active_file_path {
            path.clone()
        } else {
            ctrl.file_manager.current_dir.join(&active_name)
        };

        let is_windowed = ctrl
            .large_window
            .as_ref()
            .is_some_and(|w| w.path == save_path);
        if !is_windowed && (ui.get_is_dirty() || !ctrl.tab_buffers.contains_key(&save_path)) {
            let text = ui.get_code_text().to_string();
            ctrl.set_tab_buffer(save_path.clone(), text);
        }

        let (write_res, saved_len) = match ctrl.tab_buffers.get(&save_path) {
            Some(buf) => (std::fs::write(&save_path, buf.as_bytes()), buf.len()),
            None => {
                let text = ui.get_code_text().to_string();
                let len = text.len();
                (std::fs::write(&save_path, text.as_bytes()), len)
            }
        };

        match write_res {
            Ok(()) => {
                ctrl.file_manager.active_file_path = Some(save_path.clone());
                ctrl.untitled_drafts.remove(&save_path);
                ctrl.dirty_tabs.remove(&save_path);
                ui.set_is_dirty(false);
                let saved_name = save_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or(active_name);
                ui.set_active_file_name(SharedString::from(&saved_name));
                ui.set_active_file_path(SharedString::from(
                    save_path.to_string_lossy().to_string(),
                ));
                ctrl.append_log(&format!(
                    "[SAVE] Saved '{}' ({} bytes)",
                    save_path.display(),
                    saved_len
                ));
                ui.set_status_message(SharedString::from("Saved"));
                ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
                sync_logs(&ui, &ctrl);
                sync_tabs_ui(&ui, &ctrl);
                persist_current_session(&ui, &ctrl);
                drop(ctrl);
                refresh_files_ui(&ui, &ctrl_clone);
            }
            Err(err) => {
                ctrl.append_log(&format!("[ERROR] Save failed: {}", err));
                ui.set_status_message(SharedString::from("Save failed"));
                ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
                sync_logs(&ui, &ctrl);
            }
        }
    });

    // Save File As (Ctrl + Shift + S)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_save_file_as(move || {
        let app_weak_inner = app_weak.clone();
        let ctrl_clone_inner = Arc::clone(&ctrl_clone);
        let Some(ui) = app_weak.upgrade() else { return };
        let (active_name, old_path, start_dir, current_text) = {
            let ctrl = ctrl_clone.lock().unwrap();
            (
                ui.get_active_file_name().to_string(),
                ctrl.file_manager.active_file_path.clone(),
                ctrl.file_manager.current_dir.clone(),
                ctrl.active_document_text_cow(&ui).into_owned(),
            )
        };

        std::thread::spawn(move || {
            let mut dialog = rfd::FileDialog::new()
                .set_title("Save As (Ctrl+Shift+S)")
                .set_directory(&start_dir)
                .add_filter("Intel 8080 Assembly (*.asm)", &["asm"])
                .add_filter("All Files (*.*)", &["*"]);

            if !active_name.is_empty() {
                dialog = dialog.set_file_name(&active_name);
            } else {
                dialog = dialog.set_file_name("NewFile.asm");
            }

            if let Some(target_path) = dialog.save_file() {
                let write_res = std::fs::write(&target_path, &current_text);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = app_weak_inner.upgrade() else {
                        return;
                    };
                    let mut ctrl = ctrl_clone_inner.lock().unwrap();
                    match write_res {
                        Ok(()) => {
                            let new_file_name = target_path
                                .file_name()
                                .map(|s| s.to_string_lossy().to_string())
                                .unwrap_or_else(|| "file.asm".to_string());

                            if let Some(ref old) = old_path {
                                ctrl.untitled_drafts.remove(old);
                                ctrl.dirty_tabs.remove(old);
                                ctrl.remove_tab_buffer(old);
                                if old != &target_path {
                                    if let Some(idx) = ctrl.open_tabs.iter().position(|p| p == old)
                                    {
                                        if ctrl.open_tabs.contains(&target_path) {
                                            ctrl.open_tabs.remove(idx);
                                        } else {
                                            ctrl.open_tabs[idx] = target_path.clone();
                                        }
                                    } else if !ctrl.open_tabs.contains(&target_path) {
                                        ctrl.open_tabs.push(target_path.clone());
                                    }
                                }
                            } else if !ctrl.open_tabs.contains(&target_path) {
                                ctrl.open_tabs.push(target_path.clone());
                            }

                            ctrl.file_manager.active_file_path = Some(target_path.clone());
                            ctrl.dirty_tabs.remove(&target_path);
                            ctrl.set_tab_buffer(target_path.clone(), current_text.clone());
                            ctrl.undo_manager.reset(current_text.clone(), 0);

                            ui.set_active_file_name(SharedString::from(&new_file_name));
                            ui.set_active_file_path(SharedString::from(
                                target_path.to_string_lossy().to_string(),
                            ));
                            ui.set_is_dirty(false);

                            let ext = target_path
                                .extension()
                                .and_then(|e| e.to_str())
                                .unwrap_or("")
                                .to_ascii_lowercase();
                            let is_asm = matches!(ext.as_str(), "asm" | "inc" | "s" | "mac");
                            ui.set_is_active_asm(is_asm);

                            ctrl.append_log(&format!(
                                "[SAVE AS] Saved to '{}' ({} bytes)",
                                target_path.display(),
                                current_text.len()
                            ));
                            ui.set_status_message(SharedString::from("Saved"));
                            ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
                            sync_logs(&ui, &ctrl);
                            sync_tabs_ui(&ui, &ctrl);
                            persist_current_session(&ui, &ctrl);

                            let auto = ctrl.autocomplete.clone();
                            drop(ctrl);
                            update_highlight_and_labels(&ui, &auto, &current_text, is_asm);
                            refresh_files_ui(&ui, &ctrl_clone_inner);
                        }
                        Err(err) => {
                            ctrl.append_log(&format!("[ERROR] Save As failed: {}", err));
                            ui.set_status_message(SharedString::from("Save As failed"));
                            ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
                            sync_logs(&ui, &ctrl);
                        }
                    }
                });
            }
        });
    });

    // Open File Dialog
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_open_file_dialog(move || {
        let app_weak_inner = app_weak.clone();
        let ctrl_clone_inner = Arc::clone(&ctrl_clone);
        std::thread::spawn(move || {
            let start_dir = {
                let ctrl = ctrl_clone_inner.lock().unwrap();
                ctrl.file_manager.current_dir.clone()
            };
            if let Some(path) = rfd::FileDialog::new()
                .set_title("Open File")
                .set_directory(&start_dir)
                .add_filter(
                    "Assembly Files (*.asm, *.inc, *.mac)",
                    &["asm", "inc", "mac", "s"],
                )
                .add_filter("All Files (*.*)", &["*"])
                .pick_file()
            {
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = app_weak_inner.upgrade() else {
                        return;
                    };
                    open_file_internal(&ui, &ctrl_clone_inner, &path, false);
                });
            }
        });
    });
}
