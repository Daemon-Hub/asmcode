use crate::ui::controller::IdeController;
use crate::ui::{AppWindow, TerminalLineItem};
use slint::{Color, ModelRc, SharedString, VecModel};
use std::rc::Rc;

#[inline]
pub fn parse_log_line(line: &str) -> (&str, Color, &str, Color) {
    let bytes = line.as_bytes();
    let (time_part, rest) = if bytes.starts_with(b"[") {
        if let Some(end_bracket) = line.find(']') {
            if end_bracket < 15 && bytes.get(end_bracket + 1) == Some(&b' ') {
                (&line[..=end_bracket + 1], &line[end_bracket + 2..])
            } else {
                ("", line)
            }
        } else {
            ("", line)
        }
    } else {
        ("", line)
    };

    if rest.starts_with("[SUCCESS]") {
        (
            if time_part.is_empty() {
                "[SUCCESS] "
            } else {
                time_part
            },
            Color::from_rgb_u8(166, 227, 161),
            rest.strip_prefix("[SUCCESS] ").unwrap_or(rest),
            Color::from_rgb_u8(166, 227, 161),
        )
    } else if rest.starts_with("[ERROR]")
        || rest.starts_with("[CPM ERR]")
        || rest.contains("error:")
    {
        (
            if time_part.is_empty() {
                "[ERROR] "
            } else {
                time_part
            },
            Color::from_rgb_u8(243, 139, 168),
            rest.strip_prefix("[ERROR] ").unwrap_or(rest),
            Color::from_rgb_u8(243, 139, 168),
        )
    } else if rest.starts_with("[WARN]")
        || rest.starts_with("[WARNING]")
        || rest.contains("warning:")
    {
        (
            if time_part.is_empty() {
                "[WARN] "
            } else {
                time_part
            },
            Color::from_rgb_u8(249, 226, 175),
            rest.strip_prefix("[WARN] ").unwrap_or(rest),
            Color::from_rgb_u8(249, 226, 175),
        )
    } else if rest.starts_with("[CMD]") {
        (
            if time_part.is_empty() {
                "[CMD] "
            } else {
                time_part
            },
            Color::from_rgb_u8(137, 220, 235),
            rest.strip_prefix("[CMD] ").unwrap_or(rest),
            Color::from_rgb_u8(205, 214, 244),
        )
    } else if rest.starts_with("[CPM OUTPUT]") {
        (
            "[CPM OUT] ",
            Color::from_rgb_u8(148, 226, 213),
            rest.strip_prefix("[CPM OUTPUT] ").unwrap_or(rest),
            Color::from_rgb_u8(255, 255, 255),
        )
    } else if rest.starts_with("[EMULATOR]") {
        (
            if time_part.is_empty() {
                "[EMULATOR] "
            } else {
                time_part
            },
            Color::from_rgb_u8(203, 166, 247),
            rest.strip_prefix("[EMULATOR] ").unwrap_or(rest),
            Color::from_rgb_u8(186, 194, 222),
        )
    } else if rest.starts_with("[SYSTEM]") || rest.starts_with("[TOOLCHAIN]") {
        (
            if time_part.is_empty() {
                "[SYSTEM] "
            } else {
                time_part
            },
            Color::from_rgb_u8(137, 180, 250),
            rest,
            Color::from_rgb_u8(205, 214, 244),
        )
    } else {
        (
            time_part,
            Color::from_rgb_u8(108, 112, 134),
            rest,
            Color::from_rgb_u8(205, 214, 244),
        )
    }
}

pub fn update_terminal_lines(app: &AppWindow, logs: &[String]) {
    let mut items = Vec::with_capacity(logs.len());
    for (idx, line) in logs.iter().enumerate() {
        let (prefix, prefix_color, text, text_color) = parse_log_line(line);
        items.push(TerminalLineItem {
            line_number: (idx + 1) as i32,
            prefix: SharedString::from(prefix),
            prefix_color,
            text: SharedString::from(text),
            text_color,
        });
    }
    app.set_terminal_lines(ModelRc::from(Rc::new(VecModel::from(items))));
}

pub fn sync_logs(ui: &AppWindow, ctrl: &IdeController) {
    ui.set_output_log_text(SharedString::from(ctrl.get_combined_logs()));
    update_terminal_lines(ui, &ctrl.logs);
}
