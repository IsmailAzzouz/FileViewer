//! UI presentation components and layouts.

pub mod diagnostics;
pub mod empty_state;
pub mod status_bar;
pub mod toolbar;

pub use diagnostics::render_diagnostics;
pub use empty_state::render_empty_state;
pub use status_bar::{render_status_bar, StatusBarProps};
pub use toolbar::{render_toolbar, ToolbarProps};
