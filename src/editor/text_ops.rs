// Pure text editing algorithms optimized for zero superfluous allocations
// and safe byte-level UTF-8/ASCII boundary operations.

#[inline]
pub fn extract_prefix_at_offset(code: &str, byte_offset: usize) -> &str {
    let bytes = code.as_bytes();
    let offset = byte_offset.min(bytes.len());
    let mut start = offset;
    while start > 0 {
        let b = bytes[start - 1];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'?' || b == b'@' {
            start -= 1;
        } else {
            break;
        }
    }
    let mut end = offset;
    while end < bytes.len() {
        let b = bytes[end];
        if b.is_ascii_alphanumeric() || b == b'_' || b == b'.' || b == b'?' || b == b'@' {
            end += 1;
        } else {
            break;
        }
    }
    &code[start..end]
}

#[inline]
pub fn delete_word_left(code: &str, byte_offset: usize) -> (String, usize) {
    let bytes = code.as_bytes();
    let offset = byte_offset.min(bytes.len());

    let pre_bytes = &bytes[..offset];
    let line_start = pre_bytes
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|p| p + 1)
        .unwrap_or(0);

    if offset <= line_start {
        return (code.to_string(), offset);
    }

    let line_bytes = &bytes[line_start..offset];

    // Skip trailing whitespace on current line before cursor
    let mut end = line_bytes.len();
    while end > 0
        && (line_bytes[end - 1] == b' '
            || line_bytes[end - 1] == b'\t'
            || line_bytes[end - 1] == b'\r')
    {
        end -= 1;
    }

    let delete_rel = if end == 0 {
        0
    } else {
        let mut word_start = end;
        let is_ident = |b: u8| b.is_ascii_alphanumeric() || b == b'_';
        let first_is_ident = is_ident(line_bytes[end - 1]);
        while word_start > 0 {
            let b = line_bytes[word_start - 1];
            if b == b' ' || b == b'\t' || b == b'\r' {
                break;
            }
            if is_ident(b) != first_is_ident {
                break;
            }
            word_start -= 1;
        }
        word_start
    };

    let final_from = line_start + delete_rel;
    let mut new_code = String::with_capacity(code.len());
    new_code.push_str(&code[..final_from]);
    new_code.push_str(&code[offset..]);

    (new_code, final_from)
}

#[inline]
fn selection_line_bounds(code: &str, min_off: usize, max_off: usize) -> (usize, usize) {
    let bytes = code.as_bytes();
    let first_line_start = bytes[..min_off]
        .iter()
        .rposition(|&b| b == b'\n')
        .map(|p| p + 1)
        .unwrap_or(0);

    let last_line_end =
        if max_off > min_off && bytes.get(max_off.saturating_sub(1)) == Some(&b'\n') {
            max_off.saturating_sub(1)
        } else {
            bytes[max_off..]
                .iter()
                .position(|&b| b == b'\n')
                .map(|p| max_off + p)
                .unwrap_or(bytes.len())
        };
    (first_line_start, last_line_end)
}

#[inline]
pub fn toggle_comment_in_code(
    code: &str,
    cursor_offset: usize,
    anchor_offset: usize,
) -> (String, i32) {
    let bytes = code.as_bytes();
    let min_off = cursor_offset.min(anchor_offset).min(bytes.len());
    let max_off = cursor_offset.max(anchor_offset).min(bytes.len());

    let (first_line_start, last_line_end) = selection_line_bounds(code, min_off, max_off);

    let target_section = &code[first_line_start..last_line_end];

    let mut all_commented = true;
    let mut has_non_empty = false;
    let mut line_count = 0;
    for line in target_section.split('\n') {
        line_count += 1;
        let trimmed = line.trim_start();
        if !trimmed.is_empty() {
            has_non_empty = true;
            if !trimmed.starts_with(';') {
                all_commented = false;
            }
        }
    }
    if !has_non_empty {
        all_commented = false;
    }

    let mut result_section = String::with_capacity(target_section.len() + line_count * 2);
    for (idx, line) in target_section.split('\n').enumerate() {
        if idx > 0 {
            result_section.push('\n');
        }
        let trimmed = line.trim_start();
        let ws_len = line.len() - trimmed.len();
        let ws = &line[..ws_len];

        if all_commented {
            if let Some(uncommented) = trimmed.strip_prefix("; ") {
                result_section.push_str(ws);
                result_section.push_str(uncommented);
            } else if let Some(uncommented) = trimmed.strip_prefix(';') {
                result_section.push_str(ws);
                result_section.push_str(uncommented);
            } else {
                result_section.push_str(line);
            }
        } else if trimmed.is_empty() {
            result_section.push_str(line);
        } else {
            result_section.push_str(ws);
            result_section.push_str("; ");
            result_section.push_str(trimmed);
        }
    }

    let mut new_code = String::with_capacity(code.len() + line_count * 2);
    new_code.push_str(&code[..first_line_start]);
    new_code.push_str(&result_section);
    let new_cursor_pos = new_code.len();
    new_code.push_str(&code[last_line_end..]);

    (new_code, new_cursor_pos as i32)
}

#[inline]
pub fn apply_completion_to_code(code: &str, line_idx: usize, insert_text: &str) -> String {
    let mut target_line = None;
    for (i, line) in code.lines().enumerate() {
        if i == line_idx {
            target_line = Some(line);
            break;
        }
    }

    let Some(current_line) = target_line else {
        let mut fallback = String::with_capacity(code.len() + insert_text.len() + 12);
        fallback.push_str(code);
        fallback.push_str("\n        ");
        fallback.push_str(insert_text);
        fallback.push('\n');
        return fallback;
    };

    let trimmed = current_line.trim_end();
    let trimmed_b = trimmed.as_bytes();
    let mut word_start = 0;
    let mut pos = trimmed_b.len();
    while pos > 0 {
        let b = trimmed_b[pos - 1];
        if !b.is_ascii_alphanumeric() && b != b'_' && b != b'.' {
            word_start = pos;
            break;
        }
        pos -= 1;
    }
    let prefix = &current_line[..word_start];

    let mut result = String::with_capacity(code.len() + insert_text.len() + 16);
    for (i, line) in code.lines().enumerate() {
        if i == line_idx {
            result.push_str(prefix);
            result.push_str(insert_text);
        } else {
            result.push_str(line);
        }
        result.push('\n');
    }
    if !code.ends_with('\n') && result.ends_with('\n') {
        result.pop();
    }
    result
}

#[inline]
pub fn move_lines_up(
    code: &str,
    cursor_offset: usize,
    anchor_offset: usize,
) -> Option<(String, usize, usize)> {
    let len = code.len();
    if len == 0 {
        return None;
    }
    let cur = cursor_offset.min(len);
    let anc = anchor_offset.min(len);
    let min_off = cur.min(anc);
    let max_off = cur.max(anc);

    let first_line_start = code[..min_off].rfind('\n').map(|p| p + 1).unwrap_or(0);
    if first_line_start == 0 {
        return None;
    }

    let last_line_end = if max_off > min_off && code.as_bytes().get(max_off.saturating_sub(1)) == Some(&b'\n') {
        max_off - 1
    } else {
        code[max_off..].find('\n').map(|p| max_off + p).unwrap_or(len)
    };

    let prev_line_start = code[..first_line_start - 1].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let prev_line_end = first_line_start - 1;

    let prev_line = &code[prev_line_start..prev_line_end];
    let block = &code[first_line_start..last_line_end];

    let mut new_code = String::with_capacity(len + 1);
    new_code.push_str(&code[..prev_line_start]);
    new_code.push_str(block);
    new_code.push('\n');
    new_code.push_str(prev_line);
    new_code.push_str(&code[last_line_end..]);

    let shift = first_line_start - prev_line_start;
    let new_cur = cur.saturating_sub(shift);
    let new_anc = anc.saturating_sub(shift);

    Some((new_code, new_cur, new_anc))
}

#[inline]
pub fn move_lines_down(
    code: &str,
    cursor_offset: usize,
    anchor_offset: usize,
) -> Option<(String, usize, usize)> {
    let len = code.len();
    if len == 0 {
        return None;
    }
    let cur = cursor_offset.min(len);
    let anc = anchor_offset.min(len);
    let min_off = cur.min(anc);
    let max_off = cur.max(anc);

    let first_line_start = code[..min_off].rfind('\n').map(|p| p + 1).unwrap_or(0);
    let last_line_end = if max_off > min_off && code.as_bytes().get(max_off.saturating_sub(1)) == Some(&b'\n') {
        max_off - 1
    } else {
        code[max_off..].find('\n').map(|p| max_off + p).unwrap_or(len)
    };

    if last_line_end >= len {
        return None;
    }

    let next_line_start = last_line_end + 1;
    let next_line_end = code[next_line_start..].find('\n').map(|p| next_line_start + p).unwrap_or(len);

    let next_line = &code[next_line_start..next_line_end];
    let block = &code[first_line_start..last_line_end];

    let mut new_code = String::with_capacity(len + 1);
    new_code.push_str(&code[..first_line_start]);
    new_code.push_str(next_line);
    new_code.push('\n');
    new_code.push_str(block);
    new_code.push_str(&code[next_line_end..]);

    let shift = next_line.len() + 1;
    let new_cur = cur + shift;
    let new_anc = anc + shift;

    Some((new_code, new_cur, new_anc))
}

#[inline]
pub fn duplicate_selection_or_line(
    code: &str,
    cursor_offset: usize,
    anchor_offset: usize,
) -> (String, usize, usize) {
    let len = code.len();
    let cur = cursor_offset.min(len);
    let anc = anchor_offset.min(len);

    if cur != anc {
        let s = cur.min(anc);
        let e = cur.max(anc);
        let selected = &code[s..e];
        let duplicate = format!("\n{}", selected);
        let mut new_code = String::with_capacity(len + duplicate.len());
        new_code.push_str(&code[..e]);
        new_code.push_str(&duplicate);
        new_code.push_str(&code[e..]);

        let new_s = e + 1;
        let new_e = new_s + selected.len();
        if cur > anc {
            (new_code, new_e, new_s)
        } else {
            (new_code, new_s, new_e)
        }
    } else {
        let line_start = code[..cur].rfind('\n').map(|p| p + 1).unwrap_or(0);
        let line_end = code[cur..].find('\n').map(|p| cur + p).unwrap_or(len);
        let current_line = &code[line_start..line_end];
        let duplicate = format!("\n{}", current_line);
        let mut new_code = String::with_capacity(len + duplicate.len());
        new_code.push_str(&code[..line_end]);
        new_code.push_str(&duplicate);
        new_code.push_str(&code[line_end..]);

        let col = cur - line_start;
        let new_pos = line_end + 1 + col;
        (new_code, new_pos, new_pos)
    }
}

/// Inserts indentation spaces at the cursor, or indents every line touched by a selection.
pub fn indent_selection_or_tab(
    code: &str,
    cursor_offset: usize,
    anchor_offset: usize,
    tab_size: usize,
) -> (String, usize, usize) {
    let len = code.len();
    let cur = cursor_offset.min(len);
    let anc = anchor_offset.min(len);
    let indent_width = tab_size.clamp(1, 8);
    let spaces = "        ";
    let indent = &spaces[..indent_width];

    if cur == anc {
        let mut new_code = String::with_capacity(len + indent_width);
        new_code.push_str(&code[..cur]);
        new_code.push_str(indent);
        new_code.push_str(&code[cur..]);
        let new_pos = cur + indent_width;
        return (new_code, new_pos, new_pos);
    }

    let min_off = cur.min(anc);
    let max_off = cur.max(anc);
    let (first_line_start, last_line_end) = selection_line_bounds(code, min_off, max_off);
    let section = &code[first_line_start..last_line_end];
    let is_single_line = !section.as_bytes().contains(&b'\n');
    let line_count = section.bytes().filter(|&b| b == b'\n').count() + 1;

    let mut result_section = String::with_capacity(section.len() + line_count * indent_width);
    let mut added_for_min = 0usize;
    let mut added_for_max = 0usize;
    let mut rel_pos = 0usize;

    for (idx, line) in section.split('\n').enumerate() {
        if idx > 0 {
            result_section.push('\n');
            rel_pos += 1;
        }
        let line_abs_start = first_line_start + rel_pos;
        let should_indent = is_single_line || !line.trim_end_matches('\r').is_empty();
        if should_indent {
            result_section.push_str(indent);
            // Preserve col-0 anchor/cursor when selection begins at the start of the first line
            if line_abs_start < min_off || (line_abs_start == min_off && min_off > first_line_start) {
                added_for_min += indent_width;
            }
            if line_abs_start < max_off {
                added_for_max += indent_width;
            }
        }
        result_section.push_str(line);
        rel_pos += line.len();
    }

    let mut new_code = String::with_capacity(len + added_for_max);
    new_code.push_str(&code[..first_line_start]);
    new_code.push_str(&result_section);
    new_code.push_str(&code[last_line_end..]);

    let new_min = min_off + added_for_min;
    let new_max = max_off + added_for_max;
    if cur >= anc {
        (new_code, new_max, new_min)
    } else {
        (new_code, new_min, new_max)
    }
}

/// Removes up to `tab_size` leading spaces (or one leading tab) from the current or selected lines.
pub fn unindent_selection_or_line(
    code: &str,
    cursor_offset: usize,
    anchor_offset: usize,
    tab_size: usize,
) -> (String, usize, usize) {
    let len = code.len();
    let cur = cursor_offset.min(len);
    let anc = anchor_offset.min(len);
    let min_off = cur.min(anc);
    let max_off = cur.max(anc);
    let indent_width = tab_size.clamp(1, 8);

    let (first_line_start, last_line_end) = selection_line_bounds(code, min_off, max_off);
    let section = &code[first_line_start..last_line_end];

    let mut result_section = String::with_capacity(section.len());
    let mut removed_for_min = 0usize;
    let mut removed_for_max = 0usize;
    let mut rel_pos = 0usize;

    for (idx, line) in section.split('\n').enumerate() {
        if idx > 0 {
            result_section.push('\n');
            rel_pos += 1;
        }
        let line_abs_start = first_line_start + rel_pos;
        let line_bytes = line.as_bytes();
        let remove_len = if line_bytes.first() == Some(&b'\t') {
            1
        } else {
            line_bytes
                .iter()
                .take(indent_width)
                .take_while(|&&b| b == b' ')
                .count()
        };

        if line_abs_start < min_off {
            removed_for_min += remove_len.min(min_off - line_abs_start);
        }
        if line_abs_start < max_off {
            removed_for_max += remove_len.min(max_off - line_abs_start);
        }

        result_section.push_str(&line[remove_len..]);
        rel_pos += line.len();
    }

    let mut new_code = String::with_capacity(len);
    new_code.push_str(&code[..first_line_start]);
    new_code.push_str(&result_section);
    new_code.push_str(&code[last_line_end..]);

    let new_min = min_off.saturating_sub(removed_for_min);
    let new_max = max_off.saturating_sub(removed_for_max);
    if cur >= anc {
        (new_code, new_max, new_min)
    } else {
        (new_code, new_min, new_max)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_move_lines_up_single_line() {
        let code = "line1\nline2\nline3";
        // cursor on line2 ("line2" starts at 6)
        let (new_code, new_cur, new_anc) = move_lines_up(code, 8, 8).unwrap();
        assert_eq!(new_code, "line2\nline1\nline3");
        assert_eq!(new_cur, 2);
        assert_eq!(new_anc, 2);
    }

    #[test]
    fn test_move_lines_up_at_top() {
        let code = "line1\nline2";
        assert!(move_lines_up(code, 2, 2).is_none());
    }

    #[test]
    fn test_move_lines_down_single_line() {
        let code = "line1\nline2\nline3";
        // cursor on line2
        let (new_code, new_cur, new_anc) = move_lines_down(code, 8, 8).unwrap();
        assert_eq!(new_code, "line1\nline3\nline2");
        assert_eq!(new_cur, 14);
        assert_eq!(new_anc, 14);
    }

    #[test]
    fn test_move_lines_down_at_bottom() {
        let code = "line1\nline2";
        assert!(move_lines_down(code, 8, 8).is_none());
    }

    #[test]
    fn test_duplicate_line_no_selection() {
        let code = "mvi a, 05h\nhlt";
        let (new_code, new_cur, new_anc) = duplicate_selection_or_line(code, 4, 4);
        assert_eq!(new_code, "mvi a, 05h\nmvi a, 05h\nhlt");
        assert_eq!(new_cur, 15);
        assert_eq!(new_anc, 15);
    }

    #[test]
    fn test_duplicate_selection_down() {
        let code = "start:\n  mvi a, 05h\n  hlt";
        // select "  mvi a, 05h" from 7 to 19
        let (new_code, new_cur, new_anc) = duplicate_selection_or_line(code, 19, 7);
        assert_eq!(new_code, "start:\n  mvi a, 05h\n  mvi a, 05h\n  hlt");
        assert_eq!(new_cur, 32);
        assert_eq!(new_anc, 20);
    }

    #[test]
    fn test_extract_prefix_at_offset() {
        let code = "mvi a, 10h\ncall PRINT_MSG\nhlt";
        let offset = 18; // inside "PRINT_MSG"
        let prefix = extract_prefix_at_offset(code, offset);
        assert_eq!(prefix, "PRINT_MSG");
    }

    #[test]
    fn test_delete_word_left_ident() {
        let code = "start:  mvi a, 05h";
        // offset at end of 05h (18)
        let (new_code, new_offset) = delete_word_left(code, 18);
        assert_eq!(new_code, "start:  mvi a, ");
        assert_eq!(new_offset, 15);
    }

    #[test]
    fn test_delete_word_left_bounded_to_line() {
        let code = "line1\nline2";
        // offset at start of line2 (6)
        let (new_code, new_offset) = delete_word_left(code, 6);
        assert_eq!(new_code, "line1\nline2");
        assert_eq!(new_offset, 6);
    }

    #[test]
    fn test_toggle_comment() {
        let code = "mvi a, 10h\ncall loop";
        let (commented, _) = toggle_comment_in_code(code, 0, 5);
        assert_eq!(commented, "; mvi a, 10h\ncall loop");

        let (uncommented, _) = toggle_comment_in_code(&commented, 0, 5);
        assert_eq!(uncommented, "mvi a, 10h\ncall loop");
    }

    #[test]
    fn test_apply_completion() {
        let code = "start:\n    mv";
        let completed = apply_completion_to_code(code, 1, "mvi a, ");
        assert_eq!(completed, "start:\n    mvi a, ");
    }

    #[test]
    fn test_indent_single_cursor_inserts_spaces() {
        let code = "mvi a, 05h";
        let (new_code, cur, anc) = indent_selection_or_tab(code, 3, 3, 4);
        assert_eq!(new_code, "mvi     a, 05h");
        assert_eq!(cur, 7);
        assert_eq!(anc, 7);
    }

    #[test]
    fn test_indent_multiline_selection_with_empty_line_and_trailing_newline() {
        let code = "start:\nmov a, b\n\nmov c, d\nhlt";
        // Select from "mov a, b" (7) through trailing newline after "mov c, d" (25)
        let (new_code, cur, anc) = indent_selection_or_tab(code, 25, 7, 4);
        assert_eq!(new_code, "start:\n    mov a, b\n\n    mov c, d\nhlt");
        assert_eq!(anc, 7);
        assert_eq!(cur, 33);
    }

    #[test]
    fn test_indent_multiline_reverse_selection() {
        let code = "mvi a, 1\nmvi b, 2";
        // Reverse selection: cursor at col 2 of line 0, anchor at col 5 of line 1
        let (new_code, cur, anc) = indent_selection_or_tab(code, 2, 14, 4);
        assert_eq!(new_code, "    mvi a, 1\n    mvi b, 2");
        assert_eq!(cur, 6);
        assert_eq!(anc, 22);
    }

    #[test]
    fn test_unindent_multiline_and_single_line() {
        let code = "    mvi a, 1\n  mvi b, 2\nhlt";
        let (new_code, cur, anc) = unindent_selection_or_line(code, 22, 0, 4);
        assert_eq!(new_code, "mvi a, 1\nmvi b, 2\nhlt");
        assert_eq!(anc, 0);
        assert_eq!(cur, 16);

        let (single, s_cur, s_anc) = unindent_selection_or_line("    nop", 5, 5, 4);
        assert_eq!(single, "nop");
        assert_eq!(s_cur, 1);
        assert_eq!(s_anc, 1);
    }
}
