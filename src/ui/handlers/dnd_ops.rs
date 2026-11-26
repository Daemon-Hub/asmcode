use crate::ui::AppWindow;
use crate::ui::controller::IdeController;
use crate::ui::explorer_bridge::refresh_files_ui;
use crate::ui::handlers::open_file_internal;
use crate::ui::terminal::sync_logs;
use slint::private_unstable_api::re_exports::DataTransfer;
use slint::{ComponentHandle, Model, SharedString};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Mutex};

/// Helper to recursively copy directories or files
pub fn copy_entry(src: &Path, dest_dir: &Path) -> Result<PathBuf, std::io::Error> {
    if !dest_dir.is_dir() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Target directory does not exist",
        ));
    }
    let file_name = match src.file_name() {
        Some(name) => name,
        None => {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "Source has no valid file name",
            ));
        }
    };
    let dest_path = dest_dir.join(file_name);
    if src.is_dir() {
        copy_dir_recursive(src, &dest_path)?;
    } else {
        std::fs::copy(src, &dest_path)?;
    }
    Ok(dest_path)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), std::io::Error> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let entry_path = entry.path();
        let target = dst.join(entry.file_name());
        if entry_path.is_dir() {
            copy_dir_recursive(&entry_path, &target)?;
        } else {
            std::fs::copy(&entry_path, &target)?;
        }
    }
    Ok(())
}

fn suggest_alternative_name(orig_name: &str, dest_dir: &Path) -> String {
    let path = Path::new(orig_name);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or(orig_name);
    let ext = path.extension().and_then(|s| s.to_str());

    for i in 1..=100 {
        let candidate = match ext {
            Some(e) => format!("{}_{}.{}", stem, i, e),
            None => format!("{}_{}", stem, i),
        };
        if !dest_dir.join(&candidate).exists() {
            return candidate;
        }
    }
    format!("{}_copy", orig_name)
}

fn handle_file_drop_to_folder(
    ui: &AppWindow,
    ctrl_clone: &Arc<Mutex<IdeController>>,
    src_path: &Path,
    dest_dir: &Path,
) {
    let file_name = match src_path.file_name() {
        Some(name) => name.to_string_lossy().to_string(),
        None => return,
    };
    let target_path = dest_dir.join(&file_name);
    if target_path.exists() {
        let alternative = suggest_alternative_name(&file_name, dest_dir);
        ui.set_dnd_conflict_source_path(SharedString::from(src_path.to_string_lossy().as_ref()));
        ui.set_dnd_conflict_dest_dir(SharedString::from(dest_dir.to_string_lossy().as_ref()));
        ui.set_dnd_conflict_filename(SharedString::from(&file_name));
        ui.set_dnd_conflict_new_name(SharedString::from(&alternative));
        ui.set_show_dnd_conflict_modal(true);
    } else {
        match copy_entry(src_path, dest_dir) {
            Ok(dest) => {
                let mut ctrl = ctrl_clone.lock().unwrap();
                ctrl.append_log(&format!(
                    "[FILE] Copied '{}' to '{}'",
                    file_name,
                    dest.display()
                ));
                sync_logs(ui, &ctrl);
            }
            Err(err) => {
                let mut ctrl = ctrl_clone.lock().unwrap();
                ctrl.append_log(&format!(
                    "[ERROR] Failed to copy '{}': {}",
                    src_path.display(),
                    err
                ));
                sync_logs(ui, &ctrl);
            }
        }
        refresh_files_ui(ui, ctrl_clone);
    }
}

/// Extract file paths from a Slint DataTransfer payload
pub fn extract_paths_from_transfer(data: &DataTransfer) -> Vec<PathBuf> {
    if data.has_file_paths()
        && let Ok(iter) = data.file_paths()
    {
        let paths: Vec<PathBuf> = iter.map(|p| p.to_path_buf()).collect();
        if !paths.is_empty() {
            return paths;
        }
    }
    if data.has_plain_text()
        && let Ok(text) = data.plain_text()
    {
        let p = PathBuf::from(text.as_str());
        if p.exists() {
            return vec![p];
        }
    }
    Vec::new()
}

pub fn attach_dnd_handlers(
    app: &AppWindow,
    controller_rc: &Arc<Mutex<IdeController>>,
    winit_hub: &crate::ui::WinitEventHub,
) {
    // 1. In-app Drop to Editor callback
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_file_dropped_in_editor(move |data| {
        let Some(ui) = app_weak.upgrade() else { return };
        let paths = extract_paths_from_transfer(&data);
        for path in paths {
            if open_file_internal(&ui, &ctrl_clone, &path, false) {
                let mut ctrl = ctrl_clone.lock().unwrap();
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                ctrl.append_log(&format!("[FILE] Opened '{}' via drag and drop", file_name));
                sync_logs(&ui, &ctrl);
            }
        }
    });

    // 2. In-app Drop to Folder / Explorer callback
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_file_dropped_in_folder(move |target_folder_str, data| {
        let Some(ui) = app_weak.upgrade() else { return };
        let paths = extract_paths_from_transfer(&data);
        let dest_dir = if target_folder_str.is_empty() {
            let ctrl = ctrl_clone.lock().unwrap();
            ctrl.file_manager.current_dir.clone()
        } else {
            PathBuf::from(target_folder_str.as_str())
        };

        for path in paths {
            handle_file_drop_to_folder(&ui, &ctrl_clone, &path, &dest_dir);
        }
    });

    // 3. Helper to make internal file drag transfer
    app.on_make_file_transfer(|path_str| {
        let path = PathBuf::from(path_str.as_str());
        let mut dt = DataTransfer::default();
        if path.exists() {
            dt.set_file_paths(vec![path.clone()]);
            dt.set_plain_text(SharedString::from(path.to_string_lossy().as_ref()));
        }
        dt
    });

    // 4. Conflict modal callbacks: Replace, Rename, Cancel
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_dnd_conflict_replace(move |source_str, dest_dir_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let src = PathBuf::from(source_str.as_str());
        let dest_dir = PathBuf::from(dest_dir_str.as_str());
        let file_name = src
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
        match copy_entry(&src, &dest_dir) {
            Ok(dest) => {
                let mut ctrl = ctrl_clone.lock().unwrap();
                ctrl.append_log(&format!(
                    "[FILE] Replaced '{}' in '{}'",
                    file_name,
                    dest.display()
                ));
                sync_logs(&ui, &ctrl);
            }
            Err(err) => {
                let mut ctrl = ctrl_clone.lock().unwrap();
                ctrl.append_log(&format!(
                    "[ERROR] Failed to replace '{}': {}",
                    src.display(),
                    err
                ));
                sync_logs(&ui, &ctrl);
            }
        }
        refresh_files_ui(&ui, &ctrl_clone);
    });

    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_dnd_conflict_rename(move |source_str, dest_dir_str, new_name_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let src = PathBuf::from(source_str.as_str());
        let dest_dir = PathBuf::from(dest_dir_str.as_str());
        let target = dest_dir.join(new_name_str.as_str());
        let res = if src.is_dir() {
            copy_dir_recursive(&src, &target)
        } else {
            std::fs::copy(&src, &target).map(|_| ())
        };
        match res {
            Ok(()) => {
                let mut ctrl = ctrl_clone.lock().unwrap();
                ctrl.append_log(&format!(
                    "[FILE] Copied '{}' as '{}' to '{}'",
                    src.display(),
                    new_name_str.as_str(),
                    dest_dir.display()
                ));
                sync_logs(&ui, &ctrl);
            }
            Err(err) => {
                let mut ctrl = ctrl_clone.lock().unwrap();
                ctrl.append_log(&format!(
                    "[ERROR] Failed to copy '{}' as '{}': {}",
                    src.display(),
                    new_name_str.as_str(),
                    err
                ));
                sync_logs(&ui, &ctrl);
            }
        }
        refresh_files_ui(&ui, &ctrl_clone);
    });

    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_dnd_conflict_cancel(move || {
        let Some(ui) = app_weak.upgrade() else { return };
        let mut ctrl = ctrl_clone.lock().unwrap();
        ctrl.append_log("[FILE] Drag and drop copy cancelled by user");
        sync_logs(&ui, &ctrl);
    });

    // 5. External OS Drag and Drop Listener (Windows, Linux, macOS)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();

    struct DragHoverState {
        last_cursor_pos: (f32, f32),
        #[allow(dead_code)]
        hovered_path: Option<PathBuf>,
    }
    let state = Rc::new(RefCell::new(DragHoverState {
        last_cursor_pos: (0.0, 0.0),
        hovered_path: None,
    }));

    let state_clone = Rc::clone(&state);
    let poll_timer = Rc::new(RefCell::new(None::<slint::Timer>));
    let poll_timer_clone = Rc::clone(&poll_timer);

    use slint::winit_030::{EventResult, WinitWindowAccessor, winit};
    winit_hub.push(move |win, event| {
        match event {
            winit::event::WindowEvent::CursorMoved { position, .. } => {
                let scale = win.scale_factor();
                let logical_x = (position.x / scale as f64) as f32;
                let logical_y = (position.y / scale as f64) as f32;
                state_clone.borrow_mut().last_cursor_pos = (logical_x, logical_y);
            }
            winit::event::WindowEvent::HoveredFile(path) => {
                let file_name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_default();
                state_clone.borrow_mut().hovered_path = Some(path.clone());

                if let Some(ui) = app_weak.upgrade() {
                    ui.set_dnd_hover_filename(SharedString::from(&file_name));
                }

                let app_weak_timer = app_weak.clone();
                let state_timer = Rc::clone(&state_clone);
                let mut timer_guard = poll_timer_clone.borrow_mut();
                if timer_guard.is_none() {
                    let timer = slint::Timer::default();
                    timer.start(
                        slint::TimerMode::Repeated,
                        std::time::Duration::from_millis(40),
                        move || {
                            let Some(ui) = app_weak_timer.upgrade() else {
                                return;
                            };
                            let (cursor_x, cursor_y) = {
                                #[cfg(windows)]
                                {
                                    let mut win32_pos = None;
                                    #[repr(C)]
                                    #[derive(Default, Clone, Copy)]
                                    struct WinPoint {
                                        x: i32,
                                        y: i32,
                                    }
                                    unsafe extern "system" {
                                        fn GetCursorPos(lpPoint: *mut WinPoint) -> i32;
                                        fn ScreenToClient(
                                            hWnd: isize,
                                            lpPoint: *mut WinPoint,
                                        ) -> i32;
                                    }
                                    use slint::winit_030::winit::raw_window_handle::{
                                        HasWindowHandle, RawWindowHandle,
                                    };
                                    ui.window().with_winit_window(|winit_win| {
                                        if let Ok(handle) = winit_win.window_handle()
                                            && let RawWindowHandle::Win32(win32) = handle.as_raw()
                                        {
                                            let hwnd = win32.hwnd.get();
                                            let mut pt = WinPoint { x: 0, y: 0 };
                                            unsafe {
                                                if GetCursorPos(&mut pt) != 0
                                                    && ScreenToClient(hwnd, &mut pt) != 0
                                                {
                                                    let scale = ui.window().scale_factor();
                                                    let lx = (pt.x as f64 / scale as f64) as f32;
                                                    let ly = (pt.y as f64 / scale as f64) as f32;
                                                    win32_pos = Some((lx, ly));
                                                }
                                            }
                                        }
                                    });
                                    win32_pos
                                        .unwrap_or_else(|| state_timer.borrow().last_cursor_pos)
                                }
                                #[cfg(not(windows))]
                                {
                                    state_timer.borrow().last_cursor_pos
                                }
                            };

                            let sidebar_w = ui.get_sidebar_width();
                            if cursor_x < sidebar_w {
                                ui.set_dnd_hover_target(SharedString::from("explorer"));
                                // Sidebar file list starts around Y = 133px, item height 30px
                                let mut matched_folder = SharedString::default();
                                if cursor_y >= 133.0 {
                                    let row_idx = ((cursor_y - 133.0) / 30.0) as usize;
                                    let model = ui.get_file_list();
                                    if row_idx < model.row_count()
                                        && let Some(item) = model.row_data(row_idx)
                                        && item.is_dir
                                    {
                                        matched_folder = item.path;
                                    }
                                }
                                ui.set_dnd_target_folder_path(matched_folder);
                            } else {
                                ui.set_dnd_hover_target(SharedString::from("editor"));
                                ui.set_dnd_target_folder_path(SharedString::default());
                            }
                        },
                    );
                    *timer_guard = Some(timer);
                }
            }
            winit::event::WindowEvent::HoveredFileCancelled => {
                state_clone.borrow_mut().hovered_path = None;
                let mut timer_guard = poll_timer_clone.borrow_mut();
                if let Some(t) = timer_guard.take() {
                    t.stop();
                }
                if let Some(ui) = app_weak.upgrade() {
                    ui.set_dnd_hover_target(SharedString::default());
                    ui.set_dnd_target_folder_path(SharedString::default());
                    ui.set_dnd_hover_filename(SharedString::default());
                }
            }
            winit::event::WindowEvent::DroppedFile(path) => {
                let mut timer_guard = poll_timer_clone.borrow_mut();
                if let Some(t) = timer_guard.take() {
                    t.stop();
                }

                if let Some(ui) = app_weak.upgrade() {
                    let hover_target = ui.get_dnd_hover_target().to_string();
                    let target_folder = ui.get_dnd_target_folder_path().to_string();

                    ui.set_dnd_hover_target(SharedString::default());
                    ui.set_dnd_target_folder_path(SharedString::default());
                    ui.set_dnd_hover_filename(SharedString::default());

                    if hover_target == "explorer" {
                        let dest_dir = if target_folder.is_empty() {
                            let ctrl = ctrl_clone.lock().unwrap();
                            ctrl.file_manager.current_dir.clone()
                        } else {
                            PathBuf::from(&target_folder)
                        };
                        handle_file_drop_to_folder(&ui, &ctrl_clone, path, &dest_dir);
                    } else {
                        open_file_internal(&ui, &ctrl_clone, path, false);
                        let mut ctrl = ctrl_clone.lock().unwrap();
                        let file_name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();
                        ctrl.append_log(&format!(
                            "[FILE] Opened '{}' via drag and drop",
                            file_name
                        ));
                        sync_logs(&ui, &ctrl);
                    }
                }
            }
            _ => {}
        }
        EventResult::Propagate
    });
}
