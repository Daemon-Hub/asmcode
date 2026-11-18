use crate::ui::controller::{IdeController, persist_current_session};
use crate::ui::editor_bridge::update_highlight_and_labels;
use crate::ui::explorer_bridge::refresh_files_ui;
use crate::ui::terminal::sync_logs;
use crate::ui::{AppWindow, FileTabItem};
use slint::{Color, ComponentHandle, ModelRc, SharedString, VecModel};
use std::cell::RefCell;
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
    let active_path_str = app.get_active_file_path().to_string();
    let is_dirty = app.get_is_dirty();
    let items: Vec<FileTabItem> = ctrl
        .open_tabs
        .iter()
        .map(|p| {
            let p_str = p.to_string_lossy().to_string();
            let name = p
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "file".to_string());
            let is_active = !p_str.is_empty() && p_str == active_path_str;
            let ext = p
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let is_asm = matches!(ext.as_str(), "asm" | "inc" | "s" | "mac");
            let is_pinned = ctrl.pinned_tabs.contains(p);
            let is_deleted = ctrl.deleted_open_tabs.contains(p);
            FileTabItem {
                name: SharedString::from(name),
                path: SharedString::from(p_str),
                is_active,
                is_dirty: if is_active { is_dirty } else { false },
                is_asm,
                is_pinned,
                is_deleted,
            }
        })
        .collect();
    app.set_open_tabs(ModelRc::from(Rc::new(VecModel::from(items))));
}

pub fn open_file_internal(
    ui: &AppWindow,
    ctrl_rc: &Arc<Mutex<IdeController>>,
    path: &PathBuf,
    is_force_load: bool,
) {
    if !path.exists() {
        return;
    }

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
    let (large_opt_enabled, soft_limit_bytes, hard_limit_bytes, soft_limit_lines) = {
        let ctrl = ctrl_rc.lock().unwrap();
        (
            ctrl.session.large_file_optimizations,
            ctrl.session.large_file_soft_limit_mb * 1024 * 1024,
            ctrl.session.large_file_hard_limit_mb * 1024 * 1024,
            ctrl.session.large_file_soft_limit_lines,
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

    // 3. Determine if soft limit exceeded by byte size (> 20 MB)
    let is_large_by_size = large_opt_enabled && file_size > soft_limit_bytes;

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
            return;
        }
    };

    // 4. If not large by size, check line count threshold (> 300,000 lines)
    let is_large_by_lines = if large_opt_enabled && !is_large_by_size {
        content.as_bytes().iter().filter(|&&b| b == b'\n').count() > soft_limit_lines
    } else {
        false
    };

    let is_large_file = is_large_by_size || is_large_by_lines;
    ctrl.is_large_file_mode = is_large_file;
    ctrl.force_full_features = false;
    ui.set_is_large_file_mode(is_large_file);

    let file_name = path
        .file_name()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file.asm".to_string());

    ui.set_active_file_name(SharedString::from(&file_name));
    ui.set_active_file_path(SharedString::from(path.to_string_lossy().to_string()));
    ui.set_code_text(SharedString::from(&content));
    ui.set_is_dirty(false);
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
    });

    // Switch Tab (Click on open file tab in tab bar)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_switch_tab(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let path = PathBuf::from(path_str.as_str());
        {
            let ctrl = ctrl_clone.lock().unwrap();
            if ctrl.file_manager.active_file_path.as_ref() == Some(&path) {
                return;
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
        let active_name = ui.get_active_file_name().to_string();
        if active_name.is_empty() {
            return;
        }

        let mut ctrl = ctrl_clone.lock().unwrap();
        let text = ui.get_code_text().to_string();

        let save_path = if let Some(path) = &ctrl.file_manager.active_file_path {
            path.clone()
        } else {
            ctrl.file_manager.current_dir.join(&active_name)
        };

        match ctrl.file_manager.write_file_content(&save_path, &text) {
            Ok(()) => {
                ui.set_is_dirty(false);
                sync_tabs_ui(&ui, &ctrl);
                ctrl.append_log(&format!(
                    "[FILE] Saved '{}' ({} bytes)",
                    save_path.display(),
                    text.len()
                ));
                ui.set_status_message(SharedString::from("Saved"));
                ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
                sync_logs(&ui, &ctrl);
            }
            Err(err) => {
                ctrl.append_log(&format!("[ERROR] Save failed: {}", err));
                ui.set_status_message(SharedString::from("Save failed"));
                ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
                sync_logs(&ui, &ctrl);
            }
        }
    });

    // Save File As (Ctrl + Shift + S: creates a copy without modifying or saving the original)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_save_file_as(move || {
        let app_weak_inner = app_weak.clone();
        let ctrl_clone_inner = Arc::clone(&ctrl_clone);
        let Some(ui) = app_weak.upgrade() else { return };
        let active_name = ui.get_active_file_name().to_string();
        let current_text = ui.get_code_text().to_string();

        let start_dir = {
            let ctrl = ctrl_clone.lock().unwrap();
            ctrl.file_manager.current_dir.clone()
        };

        std::thread::spawn(move || {
            let mut dialog = rfd::FileDialog::new()
                .set_title("Save Copy As (Ctrl+Shift+S)")
                .set_directory(&start_dir)
                .add_filter("Intel 8080 Assembly (*.asm)", &["asm"])
                .add_filter("All Files (*.*)", &["*"]);

            if !active_name.is_empty() {
                dialog = dialog.set_file_name(&active_name);
            }

            if let Some(target_path) = dialog.save_file() {
                let write_res = std::fs::write(&target_path, &current_text);
                let _ = slint::invoke_from_event_loop(move || {
                    let Some(ui) = app_weak_inner.upgrade() else { return };
                    let is_inside_project = {
                        let mut ctrl = ctrl_clone_inner.lock().unwrap();
                        match write_res {
                            Ok(()) => {
                                ctrl.append_log(&format!(
                                    "[SAVE AS] Copy saved to '{}' ({} bytes). Original file untouched.",
                                    target_path.display(),
                                    current_text.len()
                                ));
                                ui.set_status_message(SharedString::from("Copy saved"));
                                ui.set_status_color(Color::from_rgb_u8(166, 227, 161));
                                sync_logs(&ui, &ctrl);
                                target_path.starts_with(&ctrl.file_manager.current_dir)
                            }
                            Err(err) => {
                                ctrl.append_log(&format!("[ERROR] Save copy failed: {}", err));
                                ui.set_status_message(SharedString::from("Save copy failed"));
                                ui.set_status_color(Color::from_rgb_u8(243, 139, 168));
                                sync_logs(&ui, &ctrl);
                                false
                            }
                        }
                    };
                    if is_inside_project {
                        refresh_files_ui(&ui, &ctrl_clone_inner);
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
        ui.set_status_message(SharedString::from("Settings saved"));
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
}
