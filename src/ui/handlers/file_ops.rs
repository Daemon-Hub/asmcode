use crate::ui::AppWindow;
use crate::ui::controller::{IdeController, persist_current_session};
use crate::ui::editor_bridge::update_highlight_and_labels;
use crate::ui::explorer_bridge::refresh_files_ui;
use crate::ui::terminal::sync_logs;
use slint::{Color, ComponentHandle, ModelRc, SharedString};
use std::cell::RefCell;
use std::path::PathBuf;
use std::process::Command;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Instant;

#[cfg(windows)]
use std::os::windows::process::CommandExt;
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x08000000;

pub fn attach_file_handlers(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
    // Close Active File
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_close_active_file(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        ctrl.file_manager.active_file_path = None;
        ui.set_active_file_name(SharedString::default());
        ui.set_active_file_path(SharedString::default());
        ui.set_code_text(SharedString::default());
        ui.set_highlight_lines(ModelRc::default());
        ui.set_label_list(ModelRc::default());
        ui.set_line_numbers_text(SharedString::default());
        ui.set_is_dirty(false);
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
            let mut ctrl = ctrl_clone.lock().unwrap();
            match ctrl.file_manager.read_file_content(&path) {
                Ok(content) => {
                    let file_name = path
                        .file_name()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "file.asm".to_string());
                    ui.set_active_file_name(SharedString::from(&file_name));
                    ui.set_active_file_path(SharedString::from(path.to_string_lossy().to_string()));
                    ui.set_code_text(SharedString::from(&content));
                    ui.set_is_dirty(false);
                    ctrl.undo_manager.reset(content.clone(), 0);

                    ctrl.append_log(&format!("[FILE] Opened '{}'", path.display()));
                    sync_logs(&ui, &ctrl);

                    let ext = path
                        .extension()
                        .and_then(|e| e.to_str())
                        .unwrap_or("")
                        .to_ascii_lowercase();
                    let is_asm = matches!(ext.as_str(), "asm" | "inc" | "s" | "mac");
                    ui.set_is_active_asm(is_asm);

                    let auto = ctrl.autocomplete.clone();
                    persist_current_session(&ui, &ctrl);
                    drop(ctrl);
                    update_highlight_and_labels(&ui, &auto, &content, is_asm);
                }
                Err(err) => {
                    ctrl.append_log(&format!("[ERROR] Could not read file: {}", err));
                    sync_logs(&ui, &ctrl);
                }
            }
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

                ctrl.append_log(&format!("[FILE] Created '{}'", new_path.display()));
                sync_logs(&ui, &ctrl);

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
        match ctrl.file_manager.delete_path(&path) {
            Ok(()) => {
                ctrl.append_log(&format!("[EXPLORER] Deleted '{}'", path.display()));
                sync_logs(&ui, &ctrl);
                if ctrl.file_manager.active_file_path.is_none() {
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
        let mut cmd = Command::new("powershell");
        #[cfg(windows)]
        cmd.creation_flags(CREATE_NO_WINDOW);
        let _ = cmd
            .arg("-NoProfile")
            .arg("-Command")
            .arg(format!(
                "Set-Clipboard -Value '{}'",
                path_str.replace('\'', "''")
            ))
            .spawn();
        ctrl.append_log(&format!(
            "[EXPLORER] Copied path to clipboard: {}",
            path_str
        ));
        sync_logs(&ui, &ctrl);
    });

    // Reveal in Windows File Explorer
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_reveal_in_explorer_requested(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        let _ = Command::new("explorer")
            .arg(format!("/select,{}", path_str))
            .spawn();
        ctrl.append_log(&format!(
            "[EXPLORER] Revealed in File Explorer: {}",
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
                .set_title("Open Intel 8080 File")
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
                    let mut ctrl = ctrl_clone_inner.lock().unwrap();
                    if let Ok(content) = ctrl.file_manager.read_file_content(&path) {
                        let file_name = path
                            .file_name()
                            .map(|s| s.to_string_lossy().to_string())
                            .unwrap_or_else(|| "file.asm".to_string());
                        ui.set_active_file_name(SharedString::from(&file_name));
                        ui.set_active_file_path(SharedString::from(
                            path.to_string_lossy().to_string(),
                        ));
                        ui.set_code_text(SharedString::from(&content));
                        ui.set_is_dirty(false);
                        ctrl.undo_manager.reset(content.clone(), 0);

                        ctrl.append_log(&format!("[FILE] Opened from disk: '{}'", path.display()));
                        sync_logs(&ui, &ctrl);

                        let ext = path
                            .extension()
                            .and_then(|e| e.to_str())
                            .unwrap_or("")
                            .to_ascii_lowercase();
                        let is_asm = matches!(ext.as_str(), "asm" | "inc" | "s" | "mac");
                        ui.set_is_active_asm(is_asm);

                        let auto = ctrl.autocomplete.clone();
                        persist_current_session(&ui, &ctrl);
                        drop(ctrl);
                        update_highlight_and_labels(&ui, &auto, &content, is_asm);
                    }
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
}
