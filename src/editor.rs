//! Text editor module with buffer management and GPUI rendering.

pub mod buffer;
pub mod view;
pub mod wrap;

pub use buffer::{
    char_index_at_x, char_index_to_byte_offset, is_word_char, word_range_in_text, TextBuffer,
    TextSnapshot,
};
pub use view::{render_editor, EditorProps};
pub use wrap::{compute_visual_rows, find_visual_row_by_offset, VisualRow};
