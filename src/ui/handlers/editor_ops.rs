use crate::editor::{
    delete_word_left, duplicate_selection_or_line, indent_selection_or_tab, move_lines_down,
    move_lines_up, toggle_comment_in_code, unindent_selection_or_line,
};
use crate::ui::controller::IdeController;
use crate::ui::editor_bridge::{
    record_editor_mutation, refresh_active_editor_highlight_and_labels, update_highlight_and_labels,
};
use crate::ui::handlers::find_ops::attach_find_handlers;
use crate::ui::handlers::tab_ops::sync_tabs_ui;
use crate::ui::{AppWindow, TextSelectionItem};
use slint::{ComponentHandle, SharedString};
use std::sync::{Arc, Mutex};

pub fn attach_editor_handlers(app: &AppWindow, controller_rc: &Arc<Mutex<IdeController>>) {
    attach_find_handlers(app, controller_rc);
    // Viewport Scrolled (Virtualization: slice visible highlight lines)
    let app_weak = app.as_weak();
    let ctrl_scroll = Arc::clone(controller_rc);
    app.on_editor_viewport_scrolled(move |scroll_line, visible_count| {
        let Some(ui) = app_weak.upgrade() else { return };
        let is_asm = ui.get_is_active_asm();
        let theme_id = ui.get_active_theme_id();
        let is_large_mode = ui.get_is_large_file_mode();

        let s_line = scroll_line.max(0) as usize;
        let v_count = (visible_count.max(20) as usize).min(100);

        let mut ctrl = ctrl_scroll.lock().unwrap();
        let active_path = ctrl.file_manager.active_file_path.clone();

        if let Some(ref active_p) = active_path
            && let Some(win) = ctrl.large_window.clone()
            && &win.path == active_p
        {
            let margin = crate::ui::controller::TEXT_WINDOW_MARGIN_LINES;
            let top_win_ok = win.start_line == 0 || s_line >= win.start_line + margin;
            let bottom_win_ok =
                win.end_line >= win.total_lines || (s_line + v_count + margin) <= win.end_line;
            if !(top_win_ok && bottom_win_ok)
                && let (Some(buf), Some(starts)) = (
                    ctrl.tab_buffers.get(active_p).map(String::as_str),
                    ctrl.tab_line_starts.get(active_p).map(Vec::as_slice),
                )
            {
                let (win_text, new_win, _) = crate::ui::controller::build_window_slice_for_path(
                    active_p.clone(),
                    buf,
                    starts,
                    s_line,
                );
                ui.set_window_read_only(new_win.read_only);
                ui.set_total_line_count(new_win.total_lines as i32);
                ui.set_text_window_start_line(new_win.start_line as i32);
                ui.set_code_text(SharedString::from(win_text));
                ctrl.large_window = Some(new_win);
            }
        }

        if let Some(cache) = &ctrl.viewport_cache
            && cache.path == active_path
            && cache.theme_id == theme_id
            && cache.is_large_mode == is_large_mode
        {
            let viewport_start = s_line;
            let viewport_end = s_line + v_count;
            let margin = 18;
            let top_ok = cache.buffer_start == 0 || viewport_start >= cache.buffer_start + margin;
            let bottom_ok =
                cache.buffer_end >= cache.total_lines || viewport_end + margin <= cache.buffer_end;
            if top_ok && bottom_ok {
                return;
            }
        }

        let (buf_start, buf_end, total_lines) = if let Some(path) = &active_path {
            if let Some(c) = ctrl.tab_buffers.get(path) {
                let line_starts = ctrl.tab_line_starts.get(path).map(Vec::as_slice);
                let total = if c.is_empty() {
                    1
                } else {
                    line_starts.map(|v| v.len().max(1)).unwrap_or(1)
                };
                let (bs, be) = crate::ui::editor_bridge::update_viewport_highlight_for_total(
                    &ui,
                    c.as_str(),
                    line_starts,
                    crate::ui::editor_bridge::ViewportRenderRequest {
                        total_lines: Some(total),
                        scroll_line: s_line,
                        visible_count: v_count,
                        is_asm,
                        force_reparse: false,
                    },
                );
                (bs, be, total)
            } else {
                let code_str = ui.get_code_text().to_string();
                let total = code_str.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1;
                let (bs, be) = crate::ui::editor_bridge::update_viewport_highlight_for_total(
                    &ui,
                    &code_str,
                    None,
                    crate::ui::editor_bridge::ViewportRenderRequest {
                        total_lines: Some(total),
                        scroll_line: s_line,
                        visible_count: v_count,
                        is_asm,
                        force_reparse: false,
                    },
                );
                (bs, be, total)
            }
        } else {
            let code_str = ui.get_code_text().to_string();
            let total = code_str.as_bytes().iter().filter(|&&b| b == b'\n').count() + 1;
            let (bs, be) = crate::ui::editor_bridge::update_viewport_highlight_for_total(
                &ui,
                &code_str,
                None,
                crate::ui::editor_bridge::ViewportRenderRequest {
                    total_lines: Some(total),
                    scroll_line: s_line,
                    visible_count: v_count,
                    is_asm,
                    force_reparse: false,
                },
            );
            (bs, be, total)
        };

        ctrl.viewport_cache = Some(crate::ui::controller::ViewportCache {
            buffer_start: buf_start,
            buffer_end: buf_end,
            total_lines,
            theme_id,
            is_large_mode,
            path: active_path,
        });
    });

    fn apply_indent_edit(
        ui: &AppWindow,
        ctrl_rc: &Arc<Mutex<IdeController>>,
        cursor_offset: i32,
        anchor_offset: i32,
        op: fn(&str, usize, usize, usize) -> (String, usize, usize),
    ) -> TextSelectionItem {
        if ui.get_window_read_only() {
            return TextSelectionItem {
                cursor_offset,
                anchor_offset,
            };
        }
        let code = ui.get_code_text().to_string();
        let tab_size = ui.get_tab_size().max(1) as usize;
        let (new_code, new_cur, new_anc) = op(
            &code,
            cursor_offset.max(0) as usize,
            anchor_offset.max(0) as usize,
            tab_size,
        );
        if new_code == code {
            return TextSelectionItem {
                cursor_offset: new_cur as i32,
                anchor_offset: new_anc as i32,
            };
        }

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let mut ctrl = ctrl_rc.lock().unwrap();
        record_editor_mutation(ui, &mut ctrl, &new_code, new_cur as i32, true);
        sync_tabs_ui(ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        refresh_active_editor_highlight_and_labels(ui, &ctrl, is_asm, true);

        TextSelectionItem {
            cursor_offset: new_cur as i32,
            anchor_offset: new_anc as i32,
        }
    }

    // Insert Tab or indent all selected lines
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_insert_tab(move |cursor_offset, anchor_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return TextSelectionItem {
                cursor_offset,
                anchor_offset,
            };
        };
        apply_indent_edit(
            &ui,
            &ctrl_clone,
            cursor_offset,
            anchor_offset,
            indent_selection_or_tab,
        )
    });

    // Unindent current or selected lines (Shift+Tab)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_unindent_lines(move |cursor_offset, anchor_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return TextSelectionItem {
                cursor_offset,
                anchor_offset,
            };
        };
        apply_indent_edit(
            &ui,
            &ctrl_clone,
            cursor_offset,
            anchor_offset,
            unindent_selection_or_line,
        )
    });

    // Insert Newline (Preserving leading indentation / Auto-indent)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_insert_newline(move |byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        if ui.get_window_read_only() {
            return byte_offset;
        }
        let code = ui.get_code_text().to_string();
        let offset = (byte_offset as usize).min(code.len());

        let pre = &code[..offset];
        let line_start = pre.rfind('\n').map(|p| p + 1).unwrap_or(0);
        let current_line_so_far = &pre[line_start..];

        let ws_len = current_line_so_far
            .bytes()
            .take_while(|&b| b == b' ' || b == b'\t')
            .count();
        let leading_ws = &current_line_so_far[..ws_len];

        let mut new_code = String::with_capacity(code.len() + 1 + ws_len);
        new_code.push_str(&code[..offset]);
        new_code.push('\n');
        new_code.push_str(leading_ws);
        new_code.push_str(&code[offset..]);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let final_pos = (offset + 1 + ws_len) as i32;
        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, final_pos, false);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        refresh_active_editor_highlight_and_labels(&ui, &ctrl, is_asm, true);

        final_pos
    });

    // Delete Word Left (Ctrl + Backspace)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_delete_word_left(move |byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        if ui.get_window_read_only() {
            return byte_offset;
        }
        let code = ui.get_code_text().to_string();
        let (new_code, final_from) = delete_word_left(&code, byte_offset as usize);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, final_from as i32, false);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        refresh_active_editor_highlight_and_labels(&ui, &ctrl, is_asm, true);

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
                ctrl.set_tab_buffer(path, snap.text.clone());
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
                ctrl.set_tab_buffer(path, snap.text.clone());
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
        if ui.get_window_read_only() {
            return cursor_offset;
        }
        let code = ui.get_code_text().to_string();
        let (new_code, new_pos) =
            toggle_comment_in_code(&code, cursor_offset as usize, anchor_offset as usize);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, new_pos, true);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        refresh_active_editor_highlight_and_labels(&ui, &ctrl, is_asm, true);

        new_pos
    });

    // Legacy single offset fallback
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_toggle_comment(move |byte_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return 0;
        };
        if ui.get_window_read_only() {
            return byte_offset;
        }
        let code = ui.get_code_text().to_string();
        let (new_code, new_pos) =
            toggle_comment_in_code(&code, byte_offset as usize, byte_offset as usize);

        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, new_pos, true);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        refresh_active_editor_highlight_and_labels(&ui, &ctrl, is_asm, true);

        new_pos
    });

    // Duplicate Selection or Line Downwards with Enter (Ctrl + D)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_duplicate_selection_or_line(move |cursor_offset, anchor_offset| {
        let Some(ui) = app_weak.upgrade() else {
            return TextSelectionItem {
                cursor_offset,
                anchor_offset,
            };
        };
        if ui.get_window_read_only() {
            return TextSelectionItem {
                cursor_offset,
                anchor_offset,
            };
        }
        let code = ui.get_code_text().to_string();
        let cur = cursor_offset.max(0) as usize;
        let anc = anchor_offset.max(0) as usize;

        let (new_code, new_cur, new_anc) = duplicate_selection_or_line(&code, cur, anc);
        ui.set_code_text(SharedString::from(&new_code));
        ui.set_is_dirty(true);

        let mut ctrl = ctrl_clone.lock().unwrap();
        record_editor_mutation(&ui, &mut ctrl, &new_code, new_cur as i32, true);
        sync_tabs_ui(&ui, &ctrl);
        let is_asm = ui.get_is_active_asm();
        refresh_active_editor_highlight_and_labels(&ui, &ctrl, is_asm, true);

        TextSelectionItem {
            cursor_offset: new_cur as i32,
            anchor_offset: new_anc as i32,
        }
    });

    // Move Line(s) Up (Alt + UpArrow)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_move_lines_up(move |cursor_offset, anchor_offset| {
        let fallback = TextSelectionItem {
            cursor_offset,
            anchor_offset,
        };
        let Some(ui) = app_weak.upgrade() else {
            return fallback;
        };
        if ui.get_window_read_only() {
            return fallback;
        }
        let code = ui.get_code_text().to_string();
        let cur = cursor_offset.max(0) as usize;
        let anc = anchor_offset.max(0) as usize;

        if let Some((new_code, new_cur, new_anc)) = move_lines_up(&code, cur, anc) {
            ui.set_code_text(SharedString::from(&new_code));
            ui.set_is_dirty(true);

            let mut ctrl = ctrl_clone.lock().unwrap();
            record_editor_mutation(&ui, &mut ctrl, &new_code, new_cur as i32, true);
            sync_tabs_ui(&ui, &ctrl);
            let is_asm = ui.get_is_active_asm();
            refresh_active_editor_highlight_and_labels(&ui, &ctrl, is_asm, true);

            TextSelectionItem {
                cursor_offset: new_cur as i32,
                anchor_offset: new_anc as i32,
            }
        } else {
            fallback
        }
    });

    // Move Line(s) Down (Alt + DownArrow)
    let ctrl_clone = Arc::clone(controller_rc);
    let app_weak = app.as_weak();
    app.on_move_lines_down(move |cursor_offset, anchor_offset| {
        let fallback = TextSelectionItem {
            cursor_offset,
            anchor_offset,
        };
        let Some(ui) = app_weak.upgrade() else {
            return fallback;
        };
        if ui.get_window_read_only() {
            return fallback;
        }
        let code = ui.get_code_text().to_string();
        let cur = cursor_offset.max(0) as usize;
        let anc = anchor_offset.max(0) as usize;

        if let Some((new_code, new_cur, new_anc)) = move_lines_down(&code, cur, anc) {
            ui.set_code_text(SharedString::from(&new_code));
            ui.set_is_dirty(true);

            let mut ctrl = ctrl_clone.lock().unwrap();
            record_editor_mutation(&ui, &mut ctrl, &new_code, new_cur as i32, true);
            sync_tabs_ui(&ui, &ctrl);
            let is_asm = ui.get_is_active_asm();
            refresh_active_editor_highlight_and_labels(&ui, &ctrl, is_asm, true);

            TextSelectionItem {
                cursor_offset: new_cur as i32,
                anchor_offset: new_anc as i32,
            }
        } else {
            fallback
        }
    });
}
