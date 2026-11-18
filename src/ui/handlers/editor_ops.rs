use crate::editor::{delete_word_left, toggle_comment_in_code};
use crate::ui::AppWindow;
use crate::ui::controller::IdeController;
use crate::ui::editor_bridge::{record_editor_mutation, update_highlight_and_labels};
use crate::ui::handlers::file_ops::sync_tabs_ui;
use slint::{ComponentHandle, SharedString};
use std::sync::{Arc, Mutex};

pub fn attach_editor_handlers(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
    // Insert Tab (Inserts 4 spaces without losing focus)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_insert_tab(move |byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let code = ui.get_code_text().to_string();
        let offset = (byte_offset as usize).min(code.len());

        let tab_spaces = "    ";
        let mut new_code = String::with_capacity(code.len() + tab_spaces.len());
        new_code.push_str(&code[..offset]);
        new_code.push_str(tab_spaces);
        new_code.push_str(&code[offset..]);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let final_pos = (offset + tab_spaces.len()) as i32;
        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, final_pos, false);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        final_pos
    });

    // Insert Newline (Preserving leading indentation / Auto-indent)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_insert_newline(move |byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let code = ui.get_code_text().to_string();
        let offset = (byte_offset as usize).min(code.len());

        let pre = &code[..offset];
        let line_start = pre.rfind('\n').map(|p| p + 1).unwrap_or(0);
        let current_line_so_far = &pre[line_start..];

        let leading_ws: String = current_line_so_far
            .chars()
            .take_while(|c| *c == ' ' || *c == '\t')
            .collect();

        let insert_chunk = format!("\n{}", leading_ws);
        let mut new_code = String::with_capacity(code.len() + insert_chunk.len());
        new_code.push_str(&code[..offset]);
        new_code.push_str(&insert_chunk);
        new_code.push_str(&code[offset..]);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let final_pos = (offset + insert_chunk.len()) as i32;
        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, final_pos, false);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        final_pos
    });

    // Delete Word Left (Ctrl + Backspace)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_delete_word_left(move |byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let code = ui.get_code_text().to_string();
        let (new_code, final_from) = delete_word_left(&code, byte_offset as usize);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, final_from as i32, false);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        final_from as i32
    });

    // Undo (Ctrl + Z)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_editor_undo(move |current_cursor| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let mut ctrl = ctrl_clone.lock().unwrap();
        if let Some(snap) = ctrl.undo_manager.undo() {
            ui.set_code_text(SharedString::from(&snap.text));
            ui.set_is_dirty(true);
            if let Some(path) = ctrl.file_manager.active_file_path.clone() {
                ctrl.tab_buffers.insert(path, snap.text.clone());
            }
            sync_tabs_ui(&ui, &ctrl);
            let is_asm = ui.get_is_active_asm();
            update_highlight_and_labels(&ui, &ctrl.autocomplete, &snap.text, is_asm);
            if snap.cursor_offset > 0 {
                snap.cursor_offset.min(snap.text.len() as i32)
            } else {
                current_cursor.min(snap.text.len() as i32)
            }
        } else {
            current_cursor
        }
    });

    // Redo (Ctrl + Y or Ctrl + Shift + Z)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_editor_redo(move |current_cursor| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let mut ctrl = ctrl_clone.lock().unwrap();
        if let Some(snap) = ctrl.undo_manager.redo() {
            ui.set_code_text(SharedString::from(&snap.text));
            ui.set_is_dirty(true);
            if let Some(path) = ctrl.file_manager.active_file_path.clone() {
                ctrl.tab_buffers.insert(path, snap.text.clone());
            }
            sync_tabs_ui(&ui, &ctrl);
            let is_asm = ui.get_is_active_asm();
            update_highlight_and_labels(&ui, &ctrl.autocomplete, &snap.text, is_asm);
            if snap.cursor_offset > 0 {
                snap.cursor_offset.min(snap.text.len() as i32)
            } else {
                current_cursor.min(snap.text.len() as i32)
            }
        } else {
            current_cursor
        }
    });

    // Toggle Comment Range (Ctrl + /)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_toggle_comment_range(move |cursor_offset, anchor_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let code = ui.get_code_text().to_string();
        let (new_code, new_pos) =
            toggle_comment_in_code(&code, cursor_offset as usize, anchor_offset as usize);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, new_pos, true);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        new_pos
    });

    // Legacy single offset fallback
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_toggle_comment(move |byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let code = ui.get_code_text().to_string();
        let (new_code, new_pos) =
            toggle_comment_in_code(&code, byte_offset as usize, byte_offset as usize);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, new_pos, true);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        new_pos
    });

    // Duplicate Line (Ctrl + D)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_duplicate_line(move |byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        let code = ui.get_code_text().to_string();
        let offset = (byte_offset as usize).min(code.len());
        let pre = &code[..offset];
        let line_start = pre.rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line_end = code[offset..]
            .find('\n')
            .map(|p| offset + p)
            .unwrap_or(code.len());
        let current_line = &code[line_start..line_end];

        let col = offset - line_start;
        let duplicate_chunk = format!("\n{}", current_line);

        let mut new_code = String::with_capacity(code.len() + duplicate_chunk.len());
        new_code.push_str(&code[..line_end]);
        new_code.push_str(&duplicate_chunk);
        new_code.push_str(&code[line_end..]);

        let new_pos = (line_end + 1 + col) as i32;
        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, new_pos, true);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        update_highlight_and_labels(&ui, &ctrl.autocomplete, &new_code, is_asm);

        new_pos
    });
}
