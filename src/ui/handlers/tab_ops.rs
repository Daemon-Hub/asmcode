use crate::ui::controller::{IdeController, persist_current_session};
use crate::ui::explorer_bridge::refresh_files_ui;
use crate::ui::handlers::explorer_ops::copy_to_clipboard;
use crate::ui::handlers::file_ops::open_file_internal;
use crate::ui::terminal::sync_logs;
use crate::ui::{AppWindow, FileTabItem};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

fn clear_empty_editor_ui(ui: &AppWindow, ctrl: &mut IdeController) {
    ctrl.file_manager.active_file_path = None;
    ctrl.cached_symbols_path = None;
    ctrl.cached_user_symbols = std::sync::Arc::new(Vec::new());
    ui.set_active_file_name(SharedString::default());
    ui.set_active_file_path(SharedString::default());
    ui.set_code_text(SharedString::default());
    ui.set_highlight_lines(ModelRc::default());
    ui.set_label_list(ModelRc::default());
    ui.set_gutter_start_line(0);
    ui.set_line_numbers_text(SharedString::default());
    ui.set_is_dirty(false);
    ctrl.undo_manager.reset(String::new(), 0);
}

pub fn sync_tabs_ui(app: &AppWindow, ctrl: &IdeController) {
    let active_path_str = app.get_active_file_path();
    let is_dirty = app.get_is_dirty();

    // Count occurrences of each raw file name across open tabs without string allocations
    let mut name_counts: HashMap<&OsStr, usize> = HashMap::with_capacity(ctrl.open_tabs.len());
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

pub fn attach_tab_handlers(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
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
            ctrl.remove_tab_buffer(&curr);
            ctrl.open_tabs.retain(|p| p != &curr);
        }
        if let Some(next_path) = ctrl.open_tabs.last().cloned() {
            drop(ctrl);
            open_file_internal(&ui, &ctrl_clone, &next_path, false);
            return;
        }
        clear_empty_editor_ui(&ui, &mut ctrl);
        ctrl.append_log("[FILE] Closed active file");
        sync_logs(&ui, &ctrl);
        sync_tabs_ui(&ui, &ctrl);
        persist_current_session(&ui, &ctrl);
    });

    // Switch Tab
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
                ctrl.set_tab_buffer(curr.clone(), current_code);
                if ui.get_is_dirty() {
                    ctrl.dirty_tabs.insert(curr);
                } else {
                    ctrl.dirty_tabs.remove(&curr);
                }
            }
        }
        open_file_internal(&ui, &ctrl_clone, &path, false);
    });

    // Close Tab
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_close_tab(move |path_str| {
        let Some(ui) = app_weak.upgrade() else { return };
        let path = PathBuf::from(path_str.as_str());

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
        ctrl.remove_tab_buffer(&path);
        let was_active = ctrl.file_manager.active_file_path.as_ref() == Some(&path);
        ctrl.open_tabs.retain(|p| p != &path);

        if was_active {
            if let Some(next_path) = ctrl.open_tabs.last().cloned() {
                drop(ctrl);
                open_file_internal(&ui, &ctrl_clone, &next_path, false);
                return;
            }
            clear_empty_editor_ui(&ui, &mut ctrl);
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
        ctrl.remove_tab_buffer(&path);
        let was_active = ctrl.file_manager.active_file_path.as_ref() == Some(&path);
        ctrl.open_tabs.retain(|p| p != &path);

        if was_active {
            if let Some(next_path) = ctrl.open_tabs.last().cloned() {
                drop(ctrl);
                open_file_internal(&ui, &ctrl_clone, &next_path, false);
                return;
            }
            clear_empty_editor_ui(&ui, &mut ctrl);
        }
        sync_tabs_ui(&ui, &ctrl);
        persist_current_session(&ui, &ctrl);
    });

    app.on_cancel_close_deleted_tab(move || {});

    // Close Other Tabs (Preserves target tab and pinned tabs)
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
        ctrl.tab_line_starts.retain(|p, _| open_set.contains(p));
        ctrl.viewport_cache = None;
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
                clear_empty_editor_ui(&ui, &mut ctrl);
            }
        }
        let open_set: HashSet<PathBuf> = ctrl.open_tabs.iter().cloned().collect();
        ctrl.untitled_drafts.retain(|p| open_set.contains(p));
        ctrl.tab_buffers.retain(|p, _| open_set.contains(p));
        ctrl.tab_line_starts.retain(|p, _| open_set.contains(p));
        ctrl.viewport_cache = None;
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
        ctrl.tab_line_starts.retain(|p, _| open_set.contains(p));
        ctrl.viewport_cache = None;
        if ctrl
            .file_manager
            .active_file_path
            .as_ref()
            .is_some_and(|active| !ctrl.pinned_tabs.contains(active))
        {
            if let Some(first_pinned) = ctrl.open_tabs.first().cloned() {
                drop(ctrl);
                open_file_internal(&ui, &ctrl_clone, &first_pinned, false);
                return;
            }
            clear_empty_editor_ui(&ui, &mut ctrl);
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
}
