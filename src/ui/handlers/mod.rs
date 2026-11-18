pub mod autocomplete_ops;
pub mod dnd_ops;
pub mod editor_ops;
pub mod file_ops;
pub mod toolchain_ops;
pub mod view_ops;

pub use autocomplete_ops::attach_autocomplete_handlers;
pub use dnd_ops::attach_dnd_handlers;
pub use editor_ops::attach_editor_handlers;
pub use file_ops::{attach_file_handlers, open_file_internal};
pub use toolchain_ops::attach_toolchain_handlers;
pub use view_ops::attach_view_handlers;
