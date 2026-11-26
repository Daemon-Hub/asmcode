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
) {
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
            return;
        }
        {
            let mut ctrl = ctrl_rc.lock().unwrap();
            if let Some(curr) = ctrl.file_manager.active_file_path.clone() {
                let current_code = ui.get_code_text().to_string();
                ctrl.set_tab_buffer(curr.clone(), current_code);
                if ui.get_is_dirty() {
                    ctrl.dirty_tabs.insert(curr);
                } else {
                    ctrl.dirty_tabs.remove(&curr);
                }
            }
        }
        matched_path.clone()
    } else {
        path.clone()
    };
    let path = &target_path_buf;

    let (is_draft, draft_content) = {
        let ctrl = ctrl_rc.lock().unwrap();
        let is_draft = ctrl.untitled_drafts.contains(path) || !path.exists();
        let content = ctrl.tab_buffers.get(path).cloned();
        (is_draft, content)
    };

    if !path.exists() && !is_draft {
        return;
    }

    let (content, file_size) = if is_draft {
        let content = draft_content.unwrap_or_default();
        let size = content.len() as u64;
        (content, size)
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
                return;
            }
        };

        let file_size = metadata.len();
        let (large_opt_enabled, hard_limit_bytes) = {
            let ctrl = ctrl_rc.lock().unwrap();
            (
                ctrl.session.large_file_optimizations,
                ctrl.session.large_file_hard_limit_mb * 1024 * 1024,
            )
        };

        if large_opt_enabled && file_size > hard_limit_bytes && !is_force_load {
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
            return;
        }

        let mut ctrl = ctrl_rc.lock().unwrap();
        let content = if let Some(buf) = ctrl.tab_buffers.get(path).cloned() {
            buf
        } else {
            match ctrl.file_manager.read_file_content(path) {
                Ok(c) => c,
                Err(err) => {
                    ctrl.append_log(&format!(
                        "[ERROR] Could not read file '{}': {}",
                        path.display(),
                        err
                    ));
                    sync_logs(ui, &ctrl);
                    return;
                }
            }
        };
        (content, file_size)
    };

    let mut ctrl = ctrl_rc.lock().unwrap();
    ctrl.set_tab_buffer(path.clone(), content.clone());

    let (large_opt_enabled, soft_limit_bytes) = (
        ctrl.session.large_file_optimizations,
        ctrl.session.large_file_soft_limit_mb * 1024 * 1024,
    );

    let is_large_file = large_opt_enabled && file_size > soft_limit_bytes;
    ctrl.is_large_file_mode = is_large_file;
    ctrl.force_full_features = false;
    ui.set_is_large_file_mode(is_large_file);

    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "NewFile.asm".to_string());

    ui.set_active_file_name(SharedString::from(&file_name));
    ui.set_active_file_path(SharedString::from(path.to_string_lossy().to_string()));
    ui.set_code_text(SharedString::from(&content));
    let is_dirty = ctrl.dirty_tabs.contains(path);
    ui.set_is_dirty(is_dirty);
    ctrl.undo_manager.reset(content.clone(), 0);

    if !ctrl.open_tabs.contains(path) {
        ctrl.open_tabs.push(path.clone());
    }
    ctrl.file_manager.active_file_path = Some(path.clone());

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let is_asm = matches!(ext.as_str(), "asm" | "inc" | "s" | "mac");
    ui.set_is_active_asm(is_asm);

    ctrl.append_log(&format!(
        "[FILE] Opened '{}' ({} bytes, {} mode)",
        path.display(),
        file_size,
        if is_large_file {
            "large file"
        } else {
            "standard"
        }
    ));
    sync_logs(ui, &ctrl);
    sync_tabs_ui(ui, &ctrl);
    persist_current_session(ui, &ctrl);

    let labels = {
        let line_starts = ctrl.tab_line_starts.get(path).map(Vec::as_slice);
        crate::ui::editor_bridge::update_highlight_and_labels_opts(
            ui,
            &ctrl.autocomplete,
            &content,
            line_starts,
            is_asm,
            true,
        )
    };
    ctrl.cached_symbols_path = Some(path.clone());
    ctrl.cached_user_symbols = std::sync::Arc::new(labels);
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
        let auto = ctrl.autocomplete.clone();
        let text = ui.get_code_text().to_string();
        let is_asm = ui.get_is_active_asm();
        ctrl.append_log("[SYSTEM] Large file mode forcibly disabled by user");
        sync_logs(&ui, &ctrl);
        drop(ctrl);
        update_highlight_and_labels(&ui, &auto, &text, is_asm);
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
        let text = ui.get_code_text().to_string();

        let save_path = if let Some(path) = &ctrl.file_manager.active_file_path {
            path.clone()
        } else {
            ctrl.file_manager.current_dir.join(&active_name)
        };

        match ctrl.file_manager.write_file_content(&save_path, &text) {
            Ok(()) => {
                ctrl.untitled_drafts.remove(&save_path);
                ctrl.dirty_tabs.remove(&save_path);
                ctrl.set_tab_buffer(save_path.clone(), text.clone());
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
                    text.len()
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
        let (active_name, old_path, start_dir) = {
            let ctrl = ctrl_clone.lock().unwrap();
            (
                ui.get_active_file_name().to_string(),
                ctrl.file_manager.active_file_path.clone(),
                ctrl.file_manager.current_dir.clone(),
            )
        };
        let current_text = ui.get_code_text().to_string();

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
