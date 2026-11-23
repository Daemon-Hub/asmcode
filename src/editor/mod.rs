pub mod find;
pub mod text_ops;
pub mod undo;

pub use find::{find_all_matches, replace_all_matches, replace_current_match};
pub use text_ops::{
    apply_completion_to_code, delete_word_left, duplicate_selection_or_line,
    extract_prefix_at_offset, move_lines_down, move_lines_up, toggle_comment_in_code,
};
