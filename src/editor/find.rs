use rayon::prelude::*;

pub fn is_word_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

#[inline]
fn check_word_boundaries(bytes: &[u8], start: usize, end: usize) -> bool {
    let prev_ok = start == 0 || !is_word_byte(bytes[start - 1]);
    let next_ok = end >= bytes.len() || !is_word_byte(bytes[end]);
    prev_ok && next_ok
}

fn find_matches_in_byte_range(
    full_bytes: &[u8],
    range_start: usize,
    range_end: usize,
    q_bytes: &[u8],
    match_case: bool,
    whole_word: bool,
) -> Vec<(usize, usize)> {
    let q_len = q_bytes.len();
    if range_end.saturating_sub(range_start) < q_len {
        return Vec::new();
    }
    let limit = range_end - q_len;
    let first_q = q_bytes[0];
    let first_q_lower = first_q.to_ascii_lowercase();
    let mut matches = Vec::new();
    let mut i = range_start;

    if match_case {
        while i <= limit {
            if full_bytes[i] == first_q
                && &full_bytes[i..i + q_len] == q_bytes
                && (!whole_word || check_word_boundaries(full_bytes, i, i + q_len))
            {
                matches.push((i, i + q_len));
                i += q_len.max(1);
                continue;
            }
            i += 1;
        }
    } else {
        while i <= limit {
            if full_bytes[i].to_ascii_lowercase() == first_q_lower
                && full_bytes[i..i + q_len].eq_ignore_ascii_case(q_bytes)
                && (!whole_word || check_word_boundaries(full_bytes, i, i + q_len))
            {
                matches.push((i, i + q_len));
                i += q_len.max(1);
                continue;
            }
            i += 1;
        }
    }

    matches
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

    // Fast path for exact-case or ASCII case-insensitive queries (parallelized with Rayon on >= 64 KB)
    if match_case || query.is_ascii() {
        const PAR_THRESHOLD: usize = 64 * 1024;
        const CHUNK_TARGET: usize = 64 * 1024;

        if bytes.len() >= PAR_THRESHOLD && !q_bytes.contains(&b'\n') {
            let mut ranges = Vec::with_capacity(bytes.len() / CHUNK_TARGET + 2);
            let mut start = 0;
            while start < bytes.len() {
                let mut end = (start + CHUNK_TARGET).min(bytes.len());
                while end < bytes.len() && bytes[end] != b'\n' {
                    end += 1;
                }
                if end < bytes.len() {
                    end += 1;
                }
                ranges.push((start, end));
                start = end;
            }

            let per_chunk: Vec<Vec<(usize, usize)>> = ranges
                .into_par_iter()
                .map(|(r_start, r_end)| {
                    find_matches_in_byte_range(bytes, r_start, r_end, q_bytes, match_case, whole_word)
                })
                .collect();

            let total: usize = per_chunk.iter().map(Vec::len).sum();
            let mut merged = Vec::with_capacity(total);
            for chunk in per_chunk {
                merged.extend_from_slice(&chunk);
            }
            return merged;
        }

        return find_matches_in_byte_range(bytes, 0, bytes.len(), q_bytes, match_case, whole_word);
    }

    // UTF-8 case-insensitive fallback for non-ASCII queries
    let q_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();
    let mut matches = Vec::new();
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

    #[test]
    fn test_parallel_find_large_buffer() {
        let line = "    MOV a, b ; marker_token\n";
        let big = line.repeat(4000);
        assert!(big.len() > 100 * 1024);

        let matches = find_all_matches(&big, "MARKER_TOKEN", false, true);
        assert_eq!(matches.len(), 4000);
    }
}
