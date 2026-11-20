pub fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

pub fn find_all_matches(
    text: &str,
    query: &str,
    match_case: bool,
    whole_word: bool,
) -> Vec<(usize, usize)> {
    if query.is_empty() || text.is_empty() {
        return Vec::new();
    }

    let bytes = text.as_bytes();
    let q_bytes = query.as_bytes();
    let q_len = q_bytes.len();
    if q_len > bytes.len() {
        return Vec::new();
    }

    let mut matches = Vec::new();

    if match_case {
        let mut i = 0;
        let limit = bytes.len() - q_len;
        while i <= limit {
            if &bytes[i..i + q_len] == q_bytes {
                let is_word_valid = if whole_word {
                    let prev_ok = if i == 0 {
                        true
                    } else {
                        !is_word_byte(bytes[i - 1])
                    };
                    let next_ok = if i + q_len >= bytes.len() {
                        true
                    } else {
                        !is_word_byte(bytes[i + q_len])
                    };
                    prev_ok && next_ok
                } else {
                    true
                };

                if is_word_valid {
                    matches.push((i, i + q_len));
                    i += q_len.max(1);
                    continue;
                }
            }
            i += 1;
        }
    } else {
        // Fast ASCII / UTF-8 case-insensitive search
        let q_lower = query.to_lowercase();
        let q_lower_bytes = q_lower.as_bytes();
        let q_len_lower = q_lower_bytes.len();

        let text_lower = text.to_lowercase();
        let text_lower_bytes = text_lower.as_bytes();

        if text_lower_bytes.len() != bytes.len() {
            // Fallback for non-ASCII length changing cases
            let mut search_start = 0;
            while let Some(pos) = text_lower[search_start..].find(&q_lower) {
                let actual_start = search_start + pos;
                let actual_end = actual_start + q_lower.len();
                let is_word_valid = if whole_word {
                    let prev_ok = if actual_start == 0 {
                        true
                    } else {
                        let prev_ch = text_lower[..actual_start].chars().next_back().unwrap_or(' ');
                        !prev_ch.is_alphanumeric() && prev_ch != '_'
                    };
                    let next_ok = if actual_end >= text_lower.len() {
                        true
                    } else {
                        let next_ch = text_lower[actual_end..].chars().next().unwrap_or(' ');
                        !next_ch.is_alphanumeric() && next_ch != '_'
                    };
                    prev_ok && next_ok
                } else {
                    true
                };

                if is_word_valid {
                    matches.push((actual_start, actual_end));
                }
                search_start = actual_end.max(search_start + 1);
            }
        } else {
            // Exact 1-to-1 byte positions
            let mut i = 0;
            let limit = bytes.len() - q_len_lower;
            while i <= limit {
                if &text_lower_bytes[i..i + q_len_lower] == q_lower_bytes {
                    let is_word_valid = if whole_word {
                        let prev_ok = if i == 0 {
                            true
                        } else {
                            !is_word_byte(bytes[i - 1])
                        };
                        let next_ok = if i + q_len_lower >= bytes.len() {
                            true
                        } else {
                            !is_word_byte(bytes[i + q_len_lower])
                        };
                        prev_ok && next_ok
                    } else {
                        true
                    };

                    if is_word_valid {
                        matches.push((i, i + q_len_lower));
                        i += q_len_lower.max(1);
                        continue;
                    }
                }
                i += 1;
            }
        }
    }

    matches
}

pub fn replace_current_match(
    text: &str,
    match_range: (usize, usize),
    replacement: &str,
) -> (String, usize) {
    let (start, end) = match_range;
    if start > end || end > text.len() {
        return (text.to_string(), start);
    }

    let mut result = String::with_capacity(text.len() + replacement.len() - (end - start));
    result.push_str(&text[..start]);
    result.push_str(replacement);
    let new_pos = result.len();
    result.push_str(&text[end..]);

    (result, new_pos)
}

pub fn replace_all_matches(
    text: &str,
    matches: &[(usize, usize)],
    replacement: &str,
) -> (String, usize) {
    if matches.is_empty() {
        return (text.to_string(), 0);
    }

    let mut result = String::with_capacity(text.len());
    let mut last_end = 0;
    let mut count = 0;

    for &(start, end) in matches {
        if start >= last_end && end <= text.len() {
            result.push_str(&text[last_end..start]);
            result.push_str(replacement);
            last_end = end;
            count += 1;
        }
    }
    result.push_str(&text[last_end..]);

    (result, count)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_find_all_case_sensitive() {
        let text = "MOV A, B\nmov b, c\nMOV C, D";
        let matches = find_all_matches(text, "MOV", true, false);
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0], (0, 3));
        assert_eq!(matches[1], (18, 21));
    }

    #[test]
    fn test_find_all_case_insensitive() {
        let text = "MOV A, B\nmov b, c\nMOV C, D";
        let matches = find_all_matches(text, "MOV", false, false);
        assert_eq!(matches.len(), 3);
    }

    #[test]
    fn test_find_whole_word() {
        let text = "mov a, b\nmove_data: nop\nmov";
        let matches_whole = find_all_matches(text, "mov", false, true);
        assert_eq!(matches_whole.len(), 2);
        assert_eq!(matches_whole[0], (0, 3));
        assert_eq!(matches_whole[1], (24, 27));

        let matches_part = find_all_matches(text, "mov", false, false);
        assert_eq!(matches_part.len(), 3);
    }

    #[test]
    fn test_replace_current() {
        let text = "mov a, b";
        let (new_text, pos) = replace_current_match(text, (0, 3), "mvi");
        assert_eq!(new_text, "mvi a, b");
        assert_eq!(pos, 3);
    }

    #[test]
    fn test_replace_all() {
        let text = "foo 123 foo 456 foo";
        let matches = find_all_matches(text, "foo", true, true);
        let (new_text, count) = replace_all_matches(text, &matches, "bar");
        assert_eq!(new_text, "bar 123 bar 456 bar");
        assert_eq!(count, 3);
    }
}
