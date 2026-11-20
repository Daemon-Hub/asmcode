use crate::ui::controller::{IdeController, persist_current_session};
use crate::ui::editor_bridge::{ensure_active_file_or_create_draft, update_highlight_and_labels};
use crate::ui::explorer_bridge::refresh_files_ui;
use crate::ui::terminal::sync_logs;
use crate::ui::{AppWindow, FileTabItem};
use slint::{Color, ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

fn copy_to_clipboard(text: &str) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(text);
    }
}

fn reveal_in_file_manager(path: &Path) {
    #[cfg(windows)]
    {
        let path_clean = path.to_string_lossy().replace('/', "\\");
        let mut cmd = Command::new("explorer");
        cmd.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd.arg(format!("/select,{}", path_clean)).spawn();
    }
    #[cfg(target_os = "macos")]
    {
        let _ = Command::new("open").arg("-R").arg(path).spawn();
    }
    #[cfg(not(any(windows, target_os = "macos")))]
    {
        let target = if path.is_file() {
            path.parent().unwrap_or(path)
        } else {
            path
        };
        let _ = Command::new("xdg-open").arg(target).spawn();
    }
}

pub fn sync_tabs_ui(app: &AppWindow, ctrl: &IdeController) {
    let active_path_str = app.get_active_file_path();
    let is_dirty = app.get_is_dirty();

    // Count occurrences of each raw file name across open tabs without string allocations
    let mut name_counts: std::collections::HashMap<&std::ffi::OsStr, usize> =
        std::collections::HashMap::with_capacity(ctrl.open_tabs.len());
    for p in &ctrl.open_tabs {
        let os_name = p.file_name().unwrap_or_default();
        *name_counts.entry(os_name).or_insert(0) += 1;
    }

    let mut items = Vec::with_capacity(ctrl.open_tabs.len());
    for p in &ctrl.open_tabs {
        let p_str = p.to_string_lossy();
        let os_name = p.file_name().unwrap_or_default();
        let raw_name = p
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());

        // Disambiguate duplicate file names with shortened parent/relative path
        let display_name = if name_counts.get(os_name).copied().unwrap_or(0) > 1 {
            let short_path = if let Ok(rel) = p.strip_prefix(&ctrl.file_manager.current_dir) {
                if let Some(parent) = rel.parent() {
                    let s = parent.to_string_lossy().replace('\\', "/");
                    if s.is_empty() { "root".to_string() } else { s }
                } else {
                    "root".to_string()
                }
            } else if let Some(parent) = p.parent() {
                let parent_name = parent
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_default();
                if parent_name.is_empty() {
                    parent.to_string_lossy().to_string()
                } else {
                    parent_name
                }
            } else {
                String::new()
            };

            if short_path.is_empty() {
                raw_name
            } else {
                format!("{} ({})", raw_name, short_path)
            }
        } else {
            raw_name
        };

        let is_active = !p_str.is_empty() && p_str.as_ref() == active_path_str.as_str();
        let ext = p
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        let is_asm = matches!(ext.as_str(), "asm" | "inc" | "s" | "mac");
        let is_pinned = ctrl.pinned_tabs.contains(p);
        let is_deleted = ctrl.deleted_open_tabs.contains(p);
        let tab_dirty = if is_active {
            is_dirty
        } else {
            ctrl.dirty_tabs.contains(p)
        };
        items.push(FileTabItem {
            name: SharedString::from(display_name),
            path: SharedString::from(p_str.as_ref()),
            is_active,
            is_dirty: tab_dirty,
            is_asm,
            is_pinned,
            is_deleted,
        });
    }
    app.set_open_tabs(ModelRc::from(Rc::new(VecModel::from(items))));
}

pub fn open_file_internal(
    ui: &AppWindow,
    ctrl_rc: &Arc<Mutex<IdeController>>,
    path: &PathBuf,
    is_force_load: bool,
) {
    // If the exact same file is already open in editor, switch to it without duplicate tabs
    let existing_tab = {
        let ctrl = ctrl_rc.lock().unwrap();
        let canonical_target = std::fs::canonicalize(path).unwrap_or_else(|_| path.clone());
        ctrl.open_tabs
            .iter()
            .find(|open_p| {
                *open_p == path
                    || std::fs::canonicalize(open_p).unwrap_or_else(|_| (*open_p).clone())
                        == canonical_target
            })
            .cloned()
    };

    let target_path_buf = if let Some(ref matched_path) = existing_tab {
        let is_already_active = {
            let ctrl = ctrl_rc.lock().unwrap();
            ctrl.file_manager.active_file_path.as_ref() == Some(matched_path)
        };
        if is_already_active {
            return;
        }
        // Save current active tab buffer before switching
        {
            let mut ctrl = ctrl_rc.lock().unwrap();
            if let Some(curr) = ctrl.file_manager.active_file_path.clone() {
                let current_code = ui.get_code_text().to_string();
                ctrl.tab_buffers.insert(curr.clone(), current_code);
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
        // 1. Inspect metadata prior to RAM buffer allocation
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
        let (large_opt_enabled, _soft_limit_bytes, hard_limit_bytes) = {
            let ctrl = ctrl_rc.lock().unwrap();
            (
                ctrl.session.large_file_optimizations,
                ctrl.session.large_file_soft_limit_mb * 1024 * 1024,
                ctrl.session.large_file_hard_limit_mb * 1024 * 1024,
            )
        };

        // 2. Hard threshold check (500 MB default): Prompt confirmation modal before reading
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
    ctrl.tab_buffers.insert(path.clone(), content.clone());

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

    let auto = ctrl.autocomplete.clone();
    drop(ctrl);
    update_highlight_and_labels(ui, &auto, &content, is_asm);
}

pub fn attach_file_handlers(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
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
        let auto = ctrl.autocomplete.clone();
        let text = ui.get_code_text().to_string();
        let is_asm = ui.get_is_active_asm();
        ctrl.append_log("[SYSTEM] Large file mode forcibly disabled by user");
        sync_logs(&ui, &ctrl);
        drop(ctrl);
        update_highlight_and_labels(&ui, &auto, &text, is_asm);
    });

    // Close Active File
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_close_active_file(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        if let Some(curr) = ctrl.file_manager.active_file_path.clone() {
            ctrl.pinned_tabs.remove(&curr);
            ctrl.deleted_open_tabs.remove(&curr);
            ctrl.untitled_drafts.remove(&curr);
            ctrl.dirty_tabs.remove(&curr);
            ctrl.tab_buffers.remove(&curr);
            ctrl.open_tabs.retain(|p| p != &curr);
        }
        if let Some(next_path) = ctrl.open_tabs.last().cloned() {
            drop(ctrl);
            open_file_internal(&ui, &ctrl_clone, &next_path, false);
            return;
        }
        ctrl.file_manager.active_file_path = None;
        ui.set_active_file_name(SharedString::default());
        ui.set_active_file_path(SharedString::default());
        ui.set_code_text(SharedString::default());
        ui.set_highlight_lines(ModelRc::default());
        ui.set_label_list(ModelRc::default());
        ui.set_line_numbers_text(SharedString::default());
        ui.set_is_dirty(false);
        ctrl.undo_manager.reset(String::new(), 0);
        sync_tabs_ui(&ui, &ctrl);
        persist_current_session(&ui, &ctrl);
    });

    // Switch Tab (Click on open file tab in tab bar)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_switch_tab(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let path = PathBuf::from(path_str.as_str());
        {
            let mut ctrl = ctrl_clone.lock().unwrap();
            if ctrl.file_manager.active_file_path.as_ref() == Some(&path) {
                return;
            }
            if let Some(curr) = ctrl.file_manager.active_file_path.clone() {
                let current_code = ui.get_code_text().to_string();
                ctrl.tab_buffers.insert(curr.clone(), current_code);
                if ui.get_is_dirty() {
                    ctrl.dirty_tabs.insert(curr);
                } else {
                    ctrl.dirty_tabs.remove(&curr);
                }
            }
        }
        open_file_internal(&ui, &ctrl_clone, &path, false);
    });

    // Close Tab (Click on 'x' button on tab)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_close_tab(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let path = PathBuf::from(path_str.as_str());

        // If file is deleted on disk, show confirmation modal to prevent accidental loss
        {
            let ctrl = ctrl_clone.lock().unwrap();
            if ctrl.deleted_open_tabs.contains(&path) {
                ui.set_pending_deleted_tab_path(SharedString::from(path_str.as_str()));
                ui.set_show_deleted_tab_modal(true);
                return;
            }
        }

        let mut ctrl = ctrl_clone.lock().unwrap();
        ctrl.pinned_tabs.remove(&path);
        ctrl.deleted_open_tabs.remove(&path);
        ctrl.untitled_drafts.remove(&path);
        ctrl.dirty_tabs.remove(&path);
        ctrl.tab_buffers.remove(&path);
        let was_active = ctrl.file_manager.active_file_path.as_ref() == Some(&path);
        ctrl.open_tabs.retain(|p| p != &path);

        if was_active {
            if let Some(next_path) = ctrl.open_tabs.last().cloned() {
                drop(ctrl);
                open_file_internal(&ui, &ctrl_clone, &next_path, false);
                return;
            } else {
                ctrl.file_manager.active_file_path = None;
                ui.set_active_file_name(SharedString::default());
                ui.set_active_file_path(SharedString::default());
                ui.set_code_text(SharedString::default());
                ui.set_highlight_lines(ModelRc::default());
                ui.set_label_list(ModelRc::default());
                ui.set_line_numbers_text(SharedString::default());
                ui.set_is_dirty(false);
                ctrl.undo_manager.reset(String::new(), 0);
            }
        }
        sync_tabs_ui(&ui, &ctrl);
        persist_current_session(&ui, &ctrl);
    });

    // Confirm Close Deleted Tab from Warning Modal
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_confirm_close_deleted_tab(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let path = PathBuf::from(path_str.as_str());
        let mut ctrl = ctrl_clone.lock().unwrap();
        ctrl.deleted_open_tabs.remove(&path);
        ctrl.pinned_tabs.remove(&path);
        ctrl.untitled_drafts.remove(&path);
        ctrl.dirty_tabs.remove(&path);
        ctrl.tab_buffers.remove(&path);
        let was_active = ctrl.file_manager.active_file_path.as_ref() == Some(&path);
        ctrl.open_tabs.retain(|p| p != &path);

        if was_active {
            if let Some(next_path) = ctrl.open_tabs.last().cloned() {
                drop(ctrl);
                open_file_internal(&ui, &ctrl_clone, &next_path, false);
                return;
            } else {
                ctrl.file_manager.active_file_path = None;
                ui.set_active_file_name(SharedString::default());
                ui.set_active_file_path(SharedString::default());
                ui.set_code_text(SharedString::default());
                ui.set_highlight_lines(ModelRc::default());
                ui.set_label_list(ModelRc::default());
                ui.set_line_numbers_text(SharedString::default());
                ui.set_is_dirty(false);
                ctrl.undo_manager.reset(String::new(), 0);
            }
        }
        sync_tabs_ui(&ui, &ctrl);
        persist_current_session(&ui, &ctrl);
    });

    // Cancel Close Deleted Tab
    let app_weak = app.as_weak();
    app.on_cancel_close_deleted_tab(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        ui.set_show_deleted_tab_modal(false);
    });

    // Close Other Tabs (Preserves pinned tabs)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_close_other_tabs(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let path = PathBuf::from(path_str.as_str());
        let mut ctrl = ctrl_clone.lock().unwrap();
        let pinned = ctrl.pinned_tabs.clone();
        ctrl.open_tabs.retain(|p| p == &path || pinned.contains(p));
        let open_set: HashSet<PathBuf> = ctrl.open_tabs.iter().cloned().collect();
        ctrl.untitled_drafts.retain(|p| open_set.contains(p));
        ctrl.tab_buffers.retain(|p, _| open_set.contains(p));
        let was_active = ctrl.file_manager.active_file_path.as_ref() == Some(&path);
        if !was_active {
            drop(ctrl);
            open_file_internal(&ui, &ctrl_clone, &path, false);
            return;
        }
        sync_tabs_ui(&ui, &ctrl);
        persist_current_session(&ui, &ctrl);
    });

    // Close Saved Tabs (Preserves dirty tabs and pinned tabs)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_close_saved_tabs(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let is_dirty = ui.get_is_dirty();
        let mut ctrl = ctrl_clone.lock().unwrap();
        let pinned = ctrl.pinned_tabs.clone();
        if is_dirty {
            let active = ctrl.file_manager.active_file_path.clone();
            ctrl.open_tabs
                .retain(|p| Some(p) == active.as_ref() || pinned.contains(p));
        } else {
            ctrl.open_tabs.retain(|p| pinned.contains(p));
            if ctrl.open_tabs.is_empty() {
                ctrl.file_manager.active_file_path = None;
                ui.set_active_file_name(SharedString::default());
                ui.set_active_file_path(SharedString::default());
                ui.set_code_text(SharedString::default());
                ui.set_highlight_lines(ModelRc::default());
                ui.set_label_list(ModelRc::default());
                ui.set_line_numbers_text(SharedString::default());
                ui.set_is_dirty(false);
                ctrl.undo_manager.reset(String::new(), 0);
            }
        }
        let open_set: HashSet<PathBuf> = ctrl.open_tabs.iter().cloned().collect();
        ctrl.untitled_drafts.retain(|p| open_set.contains(p));
        ctrl.tab_buffers.retain(|p, _| open_set.contains(p));
        sync_tabs_ui(&ui, &ctrl);
        persist_current_session(&ui, &ctrl);
    });

    // Close All Tabs (Preserves pinned tabs)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_close_all_tabs(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        let pinned = ctrl.pinned_tabs.clone();
        ctrl.open_tabs.retain(|p| pinned.contains(p));
        let open_set: HashSet<PathBuf> = ctrl.open_tabs.iter().cloned().collect();
        ctrl.untitled_drafts.retain(|p| open_set.contains(p));
        ctrl.tab_buffers.retain(|p, _| open_set.contains(p));
        if let Some(_active) = ctrl
            .file_manager
            .active_file_path
            .clone()
            .filter(|active| !ctrl.pinned_tabs.contains(active))
        {
            if let Some(first_pinned) = ctrl.open_tabs.first().cloned() {
                drop(ctrl);
                open_file_internal(&ui, &ctrl_clone, &first_pinned, false);
                return;
            } else {
                ctrl.file_manager.active_file_path = None;
                ui.set_active_file_name(SharedString::default());
                ui.set_active_file_path(SharedString::default());
                ui.set_code_text(SharedString::default());
                ui.set_highlight_lines(ModelRc::default());
                ui.set_label_list(ModelRc::default());
                ui.set_line_numbers_text(SharedString::default());
                ui.set_is_dirty(false);
                ctrl.undo_manager.reset(String::new(), 0);
            }
        }
        sync_tabs_ui(&ui, &ctrl);
        persist_current_session(&ui, &ctrl);
    });

    // Copy Relative Path
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_copy_relative_path(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let path = PathBuf::from(path_str.as_str());
        let mut ctrl = ctrl_clone.lock().unwrap();
        let rel_str = path
            .strip_prefix(&ctrl.file_manager.current_dir)
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|_| path_str.to_string());
        copy_to_clipboard(&rel_str);
        ctrl.append_log(&format!(
            "[TAB] Copied relative path to clipboard: {}",
            rel_str
        ));
        sync_logs(&ui, &ctrl);
    });

    // Reveal in Explorer Tree
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_show_in_explorer_tree(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let path = PathBuf::from(path_str.as_str());
        let mut ctrl = ctrl_clone.lock().unwrap();
        ctrl.file_manager.expand_parents_of(&path);
        let fname = path
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_default();
        ui.set_selected_explorer_path(SharedString::from(path.to_string_lossy().to_string()));
        ui.set_selected_explorer_name(SharedString::from(fname));
        ctrl.append_log(&format!("[EXPLORER] Revealed in tree: {}", path.display()));
        sync_logs(&ui, &ctrl);
        drop(ctrl);
        refresh_files_ui(&ui, &ctrl_clone);
    });

    // Toggle Pin Tab
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_toggle_pin_tab(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let path = PathBuf::from(path_str.as_str());
        let mut ctrl = ctrl_clone.lock().unwrap();
        if ctrl.pinned_tabs.contains(&path) {
            ctrl.pinned_tabs.remove(&path);
            ctrl.append_log(&format!("[TAB] Tab unpinned: {}", path_str));
        } else {
            ctrl.pinned_tabs.insert(path);
            ctrl.append_log(&format!("[TAB] Tab pinned: {}", path_str));
        }
        sync_tabs_ui(&ui, &ctrl);
        sync_logs(&ui, &ctrl);
        persist_current_session(&ui, &ctrl);
    });

    // Folder Double-Click State Tracker: (last_path, last_click_instant)
    let last_folder_click: Rc<RefCell<Option<(PathBuf, Instant)>>> = Rc::new(RefCell::new(None));

    // File Clicked in Sidebar (Single-click opens file; Click toggles tree folder expansion)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    let folder_click_clone = Rc::clone(&last_folder_click);
    app.on_file_clicked(move |path_str, is_dir| {
        let Some(ui) = app_weak.upgrade() else { return };
        let path = PathBuf::from(path_str.as_str());

        if is_dir {
            let mut ctrl = ctrl_clone.lock().unwrap();
            ctrl.file_manager.toggle_folder(&path);
            drop(ctrl);
            refresh_files_ui(&ui, &ctrl_clone);
        } else {
            *folder_click_clone.borrow_mut() = None;
            open_file_internal(&ui, &ctrl_clone, &path, false);
        }
    });

    // Navigate Parent
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_navigate_parent(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        if ctrl.file_manager.navigate_parent() {
            persist_current_session(&ui, &ctrl);
            drop(ctrl);
            refresh_files_ui(&ui, &ctrl_clone);
        }
    });

    // Refresh Files
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_refresh_files(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        refresh_files_ui(&ui, &ctrl_clone);
    });

    // Create New File
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_create_file(move |name| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        match ctrl.file_manager.create_new_file(name.as_str()) {
            Ok((new_path, initial_content)) => {
                let file_name = new_path
                    .file_name()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "new.asm".to_string());

                ui.set_active_file_name(SharedString::from(&file_name));
                ui.set_active_file_path(SharedString::from(new_path.to_string_lossy().to_string()));
                ui.set_code_text(SharedString::from(&initial_content));
                ui.set_is_dirty(false);
                ui.set_is_active_asm(true);
                ctrl.undo_manager.reset(initial_content.clone(), 0);
                if !ctrl.open_tabs.contains(&new_path) {
                    ctrl.open_tabs.push(new_path.clone());
                }
                ctrl.file_manager.active_file_path = Some(new_path.clone());

                ctrl.append_log(&format!("[FILE] Created '{}'", new_path.display()));
                sync_logs(&ui, &ctrl);
                sync_tabs_ui(&ui, &ctrl);

                let auto = ctrl.autocomplete.clone();
                drop(ctrl);
                refresh_files_ui(&ui, &ctrl_clone);
                update_highlight_and_labels(&ui, &auto, &initial_content, true);
            }
            Err(err) => {
                ctrl.append_log(&format!("[ERROR] Could not create file: {}", err));
                sync_logs(&ui, &ctrl);
            }
        }
    });

    // Create New Folder
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_create_new_folder(move |name| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        match ctrl.file_manager.create_new_folder(name.as_str()) {
            Ok(new_folder) => {
                ctrl.append_log(&format!(
                    "[EXPLORER] Created directory '{}'",
                    new_folder.display()
                ));
                sync_logs(&ui, &ctrl);
                persist_current_session(&ui, &ctrl);
                drop(ctrl);
                refresh_files_ui(&ui, &ctrl_clone);
            }
            Err(err) => {
                ctrl.append_log(&format!("[ERROR] Could not create folder: {}", err));
                sync_logs(&ui, &ctrl);
            }
        }
    });

    // Rename File or Folder
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_rename_file(move |old_path_str, new_name| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        let old_path = PathBuf::from(old_path_str.as_str());
        match ctrl.file_manager.rename_path(&old_path, new_name.as_str()) {
            Ok(new_path) => {
                ctrl.append_log(&format!("[EXPLORER] Renamed to '{}'", new_path.display()));
                sync_logs(&ui, &ctrl);
                if let Some(ref active) = ctrl.file_manager.active_file_path {
                    let fname = active
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    ui.set_active_file_name(SharedString::from(&fname));
                    ui.set_active_file_path(SharedString::from(
                        active.to_string_lossy().to_string(),
                    ));
                }
                persist_current_session(&ui, &ctrl);
                drop(ctrl);
                refresh_files_ui(&ui, &ctrl_clone);
            }
            Err(err) => {
                ctrl.append_log(&format!("[ERROR] Could not rename: {}", err));
                sync_logs(&ui, &ctrl);
            }
        }
    });

    // Delete File or Folder
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_delete_file_requested(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        let path = PathBuf::from(path_str.as_str());
        let was_open = ctrl
            .open_tabs
            .iter()
            .any(|p| p == &path || p.starts_with(&path));
        let was_active = ctrl
            .file_manager
            .active_file_path
            .as_ref()
            .map(|p| p == &path || p.starts_with(&path))
            .unwrap_or(false);
        let saved_active = ctrl.file_manager.active_file_path.clone();

        match ctrl.file_manager.delete_path(&path) {
            Ok(()) => {
                ctrl.append_log(&format!("[EXPLORER] Deleted '{}'", path.display()));
                sync_logs(&ui, &ctrl);

                if was_open {
                    let affected: Vec<PathBuf> = ctrl
                        .open_tabs
                        .iter()
                        .filter(|tab| *tab == &path || tab.starts_with(&path))
                        .cloned()
                        .collect();
                    for tab in affected {
                        ctrl.deleted_open_tabs.insert(tab);
                    }
                    if was_active {
                        ctrl.file_manager.active_file_path = saved_active;
                    }
                    sync_tabs_ui(&ui, &ctrl);
                } else if ctrl.open_tabs.is_empty() {
                    ui.set_code_text(SharedString::default());
                    ui.set_active_file_name(SharedString::default());
                    ui.set_active_file_path(SharedString::default());
                    ui.set_highlight_lines(ModelRc::default());
                    ui.set_label_list(ModelRc::default());
                    ui.set_line_numbers_text(SharedString::default());
                }

                persist_current_session(&ui, &ctrl);
                drop(ctrl);
                refresh_files_ui(&ui, &ctrl_clone);
            }
            Err(err) => {
                ctrl.append_log(&format!("[ERROR] Could not delete: {}", err));
                sync_logs(&ui, &ctrl);
            }
        }
    });

    // Copy Path to Clipboard
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_copy_path_requested(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        copy_to_clipboard(&path_str);
        ctrl.append_log(&format!(
            "[EXPLORER] Copied path to clipboard: {}",
            path_str
        ));
        sync_logs(&ui, &ctrl);
    });

    // Reveal in System File Manager
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_reveal_in_explorer_requested(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        let path = PathBuf::from(path_str.as_str());
        reveal_in_file_manager(&path);
        ctrl.append_log(&format!(
            "[EXPLORER] Revealed in File Manager: {}",
            path_str
        ));
        sync_logs(&ui, &ctrl);
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
                ctrl.tab_buffers.insert(save_path.clone(), text.clone());
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
                    "[FILE] Saved '{}' ({} bytes)",
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
        let (active_name, old_path) = {
            let ctrl = ctrl_clone.lock().unwrap();
            let name = ui.get_active_file_name().to_string();
            let old = ctrl.file_manager.active_file_path.clone();
            (name, old)
        };
        let current_text = ui.get_code_text().to_string();

        let start_dir = {
            let ctrl = ctrl_clone.lock().unwrap();
            ctrl.file_manager.current_dir.clone()
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
                                ctrl.tab_buffers.remove(old);
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
                            ctrl.tab_buffers
                                .insert(target_path.clone(), current_text.clone());
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

    // Open File Dialog (Windows Native Modern File Picker - Asynchronous via rfd, No Console Window)
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

    // Open Folder Dialog (Windows Native Modern Folder Picker - Asynchronous via rfd, No Console Window)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_open_folder_dialog(move || {
        let app_weak_inner = app_weak.clone();
        let ctrl_clone_inner = Arc::clone(&ctrl_clone);
        std::thread::spawn(move || {
            let start_dir = {
                let ctrl = ctrl_clone_inner.lock().unwrap();
                ctrl.file_manager.current_dir.clone()
            };
            if let Some(folder_path) = rfd::FileDialog::new()
                .set_title("Open Workspace Folder")
                .set_directory(&start_dir)
                .pick_folder()
            {
                let folder_str = folder_path.to_string_lossy().to_string();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = app_weak_inner.upgrade() else {
                        return;
                    };
                    let mut ctrl = ctrl_clone_inner.lock().unwrap();
                    ctrl.file_manager.current_dir = folder_path;
                    ctrl.append_log(&format!(
                        "[WORKSPACE] Switched directory to: '{}'",
                        folder_str
                    ));
                    sync_logs(&ui, &ctrl);
                    persist_current_session(&ui, &ctrl);
                    drop(ctrl);
                    refresh_files_ui(&ui, &ctrl_clone_inner);
                });
            }
        });
    });

    // Undo Delete in Explorer (Ctrl+Z or context menu)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_explorer_undo(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        match ctrl.file_manager.undo_delete() {
            Ok(Some(restored)) => {
                let affected: Vec<PathBuf> = ctrl
                    .open_tabs
                    .iter()
                    .filter(|tab| *tab == &restored || tab.starts_with(&restored))
                    .cloned()
                    .collect();
                for tab in affected {
                    ctrl.deleted_open_tabs.remove(&tab);
                }
                sync_tabs_ui(&ui, &ctrl);
                ctrl.append_log(&format!(
                    "[EXPLORER] Restored deleted item: {}",
                    restored.display()
                ));
                ui.set_status_message(SharedString::from("Restored deleted item"));
                ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
                sync_logs(&ui, &ctrl);
                drop(ctrl);
                refresh_files_ui(&ui, &ctrl_clone);
            }
            Ok(None) => {
                ctrl.append_log("[EXPLORER] No deleted items to restore in history");
                sync_logs(&ui, &ctrl);
            }
            Err(err) => {
                ctrl.append_log(&format!("[ERROR] Could not restore deleted item: {}", err));
                ui.set_status_message(SharedString::from("Restore failed"));
                ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
                sync_logs(&ui, &ctrl);
            }
        }
    });

    // Save Settings
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_save_settings(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        ctrl.append_log("[SETTINGS] Configuration updated and persisted");
        let lang = ui.get_current_language().to_string();
        crate::ui::editor_bridge::populate_docs_ui(&ui, &lang);
        let msg = if lang == "ru" {
            "Настройки сохранены"
        } else {
            "Settings saved"
        };
        ui.set_status_message(SharedString::from(msg));
        ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
        sync_logs(&ui, &ctrl);
        persist_current_session(&ui, &ctrl);
    });

    // Reset Settings Defaults
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_reset_settings_defaults(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        ui.set_editor_font_size(18.0);
        ui.set_editor_font_italic(true);
        ui.set_active_theme_id(1);
        let mut ctrl = ctrl_clone.lock().unwrap();
        ctrl.append_log(
            "[SETTINGS] Defaults restored (18px, italic directives, Cyberpunk 2077 Red)",
        );
        sync_logs(&ui, &ctrl);
        persist_current_session(&ui, &ctrl);
    });

    // Browse Compiler Executable (rfd FileDialog)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_browse_compiler(move || {
        let app_weak_inner = app_weak.clone();
        let ctrl_clone_inner = Arc::clone(&ctrl_clone);
        std::thread::spawn(move || {
            let mut dialog =
                rfd::FileDialog::new().set_title("Select Intel 8080 Compiler Executable");
            #[cfg(windows)]
            {
                dialog = dialog.add_filter(
                    "Executable (*.exe, *.com, *.bat)",
                    &["exe", "com", "bat", "cmd"],
                );
            }
            dialog = dialog.add_filter("All Files (*.*)", &["*"]);

            if let Some(path) = dialog.pick_file() {
                let path_str = path.to_string_lossy().to_string();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = app_weak_inner.upgrade() else {
                        return;
                    };
                    ui.set_compiler_path(SharedString::from(&path_str));
                    let mut ctrl = ctrl_clone_inner.lock().unwrap();
                    ctrl.append_log(&format!("[TOOLCHAIN] Selected compiler: {}", path_str));
                    sync_logs(&ui, &ctrl);
                    persist_current_session(&ui, &ctrl);
                });
            }
        });
    });

    // Browse Emulator Executable (rfd FileDialog)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_browse_emulator(move || {
        let app_weak_inner = app_weak.clone();
        let ctrl_clone_inner = Arc::clone(&ctrl_clone);
        std::thread::spawn(move || {
            let mut dialog = rfd::FileDialog::new().set_title("Select CP/M Emulator Executable");
            #[cfg(windows)]
            {
                dialog = dialog.add_filter(
                    "Executable (*.exe, *.com, *.bat)",
                    &["exe", "com", "bat", "cmd"],
                );
            }
            dialog = dialog.add_filter("All Files (*.*)", &["*"]);

            if let Some(path) = dialog.pick_file() {
                let path_str = path.to_string_lossy().to_string();
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = app_weak_inner.upgrade() else {
                        return;
                    };
                    ui.set_emulator_path(SharedString::from(&path_str));
                    let mut ctrl = ctrl_clone_inner.lock().unwrap();
                    ctrl.append_log(&format!("[TOOLCHAIN] Selected emulator: {}", path_str));
                    sync_logs(&ui, &ctrl);
                    persist_current_session(&ui, &ctrl);
                });
            }
        });
    });

    // Toggle File Association for .asm
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_toggle_associate_asm(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let new_state = ui.get_associate_asm_files();
        match crate::fs_manager::associations::set_asm_association(new_state) {
            Ok(()) => {
                let mut ctrl = ctrl_clone.lock().unwrap();
                ctrl.session.associate_asm_files = new_state;
                let msg = if new_state {
                    "[SYSTEM] Registered .asm file association with ASM Code in OS"
                } else {
                    "[SYSTEM] Removed .asm file association in OS"
                };
                ctrl.append_log(msg);
                sync_logs(&ui, &ctrl);
                persist_current_session(&ui, &ctrl);
            }
            Err(e) => {
                ui.set_associate_asm_files(!new_state);
                let mut ctrl = ctrl_clone.lock().unwrap();
                ctrl.append_log(&format!("[ERROR] File association failed: {}", e));
                sync_logs(&ui, &ctrl);
            }
        }
    });
}
