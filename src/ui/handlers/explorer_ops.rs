use crate::ui::AppWindow;
use crate::ui::controller::{IdeController, persist_current_session};
use crate::ui::editor_bridge::update_highlight_and_labels;
use crate::ui::explorer_bridge::refresh_files_ui;
use crate::ui::handlers::file_ops::open_file_internal;
use crate::ui::handlers::tab_ops::sync_tabs_ui;
use crate::ui::terminal::sync_logs;
use slint::{Color, ComponentHandle, ModelRc, SharedString};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex};

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn copy_to_clipboard(text: &str) {
    if let Ok(mut clipboard) = arboard::Clipboard::new() {
        let _ = clipboard.set_text(text);
    }
}

pub fn reveal_in_file_manager(path: &Path) {
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

pub fn attach_explorer_handlers(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
    // File or Folder Clicked in Sidebar
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_file_clicked(move |path_str, is_dir| {
        let Some(ui) = app_weak.upgrade() else { return };
        let path = PathBuf::from(path_str.as_str());

        if is_dir {
            let mut ctrl = ctrl_clone.lock().unwrap();
            ctrl.file_manager.toggle_folder(&path);
            drop(ctrl);
            refresh_files_ui(&ui, &ctrl_clone);
        } else {
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
                ctrl.set_tab_buffer(new_path.clone(), initial_content.clone());

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
                ctrl.remap_path_prefix(&old_path, &new_path);
                ctrl.append_log(&format!("[EXPLORER] Renamed to '{}'", new_path.display()));
                sync_logs(&ui, &ctrl);

                let mut active_was_renamed = false;
                if let Some(ref active) = ctrl.file_manager.active_file_path {
                    active_was_renamed = active == &new_path || active.starts_with(&new_path);
                    let fname = active
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .to_string();
                    ui.set_active_file_name(SharedString::from(&fname));
                    ui.set_active_file_path(SharedString::from(
                        active.to_string_lossy().to_string(),
                    ));
                    let ext = active
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let is_asm = matches!(ext.as_str(), "asm" | "inc" | "s" | "mac");
                    ui.set_is_active_asm(is_asm);
                }

                crate::ui::handlers::tab_ops::sync_tabs_ui(&ui, &ctrl);
                persist_current_session(&ui, &ctrl);

                if active_was_renamed {
                    let is_asm = ui.get_is_active_asm();
                    let code = ui.get_code_text().to_string();
                    crate::ui::editor_bridge::update_highlight_and_labels(
                        &ui,
                        &ctrl.autocomplete,
                        &code,
                        is_asm,
                    );
                }

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
            .is_some_and(|p| p == &path || p.starts_with(&path));
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

    // Open Folder Dialog
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

    // Undo Delete in Explorer
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
}
