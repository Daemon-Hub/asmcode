pub mod text_ops;
pub mod undo;

pub use text_ops::{
    apply_completion_to_code, delete_word_left, extract_prefix_at_offset, toggle_comment_in_code,
};
