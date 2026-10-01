//! Design tokens and theme system.
//!
//! Provides WCAG compliant color palettes, semantic tokens, and layout constants.
//! Eliminates magic numbers and inline styles across UI components.

use gpui::{px, Hsla, Pixels};

// Helper macro for creating const Hsla
macro_rules! const_hsla {
    ($h:expr, $s:expr, $l:expr, $a:expr) => {
        Hsla {
            h: $h,
            s: $s,
            l: $l,
            a: $a,
        }
    };
}

// --- Surface & Background Colors ---
pub const BG_APP: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.09, 1.0); // #16161a
pub const BG_PANEL: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.12, 1.0); // #1d1d22
pub const BG_PANEL_HEADER: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.14, 1.0); // #222228
pub const BG_CARD: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.16, 1.0); // #27272f
pub const BG_GUTTER: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.11, 1.0); // #1a1a1f
pub const BG_TOOLBAR: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.13, 1.0); // #202026
pub const BG_STATUS_BAR: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.10, 1.0); // #18181d

// --- Interactive Element Colors ---
pub const BTN_BG_NORMAL: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.18, 1.0);
pub const BTN_BG_HOVER: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.24, 1.0);
pub const BTN_BG_ACTIVE: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.28, 1.0);
pub const BTN_PRIMARY_BG: Hsla = const_hsla!(217.0 / 360.0, 0.91, 0.60, 1.0); // #3b82f6
pub const BTN_PRIMARY_HOVER: Hsla = const_hsla!(217.0 / 360.0, 0.91, 0.52, 1.0);
pub const BTN_PRIMARY_ACTIVE: Hsla = const_hsla!(217.0 / 360.0, 0.91, 0.45, 1.0);
pub const BTN_PRIMARY_TEXT: Hsla = const_hsla!(0.0, 0.0, 1.0, 1.0);

// --- Borders & Separators ---
pub const BORDER_SUBTLE: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.20, 1.0); // #303038
pub const BORDER_FOCUS: Hsla = const_hsla!(217.0 / 360.0, 0.91, 0.60, 1.0); // #3b82f6

// --- Typography Colors (High Contrast) ---
pub const TEXT_PRIMARY: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.95, 1.0); // #f2f2f4
pub const TEXT_SECONDARY: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.72, 1.0); // #b4b4bd
pub const TEXT_MUTED: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.48, 1.0); // #777782
pub const TEXT_ACCENT: Hsla = const_hsla!(217.0 / 360.0, 0.91, 0.65, 1.0); // #4f92f7

// --- Syntax Highlighting Colors ---
pub const SYNTAX_KEY: Hsla = const_hsla!(199.0 / 360.0, 0.89, 0.64, 1.0); // #56c2f5 (sky)
pub const SYNTAX_STRING: Hsla = const_hsla!(142.0 / 360.0, 0.71, 0.60, 1.0); // #52d67d (emerald)
pub const SYNTAX_NUMBER: Hsla = const_hsla!(43.0 / 360.0, 0.96, 0.64, 1.0); // #fcd153 (amber)
pub const SYNTAX_BOOLEAN: Hsla = const_hsla!(271.0 / 360.0, 0.85, 0.72, 1.0); // #bc83f8 (purple)
pub const SYNTAX_NULL: Hsla = const_hsla!(0.0 / 360.0, 0.84, 0.68, 1.0); // #f46a6a (rose/red)
pub const SYNTAX_PUNCTUATION: Hsla = const_hsla!(220.0 / 360.0, 0.14, 0.75, 1.0); // #b7becc (slate)
pub const SYNTAX_COMMENT: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.50, 1.0); // #7f7f89 (grey)

// --- Editor Highlights ---
pub const LINE_ACTIVE_BG: Hsla = const_hsla!(240.0 / 360.0, 0.05, 0.15, 0.6);
pub const SELECTION_BG: Hsla = const_hsla!(217.0 / 360.0, 0.80, 0.45, 0.45);
pub const SEARCH_MATCH_BG: Hsla = const_hsla!(43.0 / 360.0, 0.95, 0.50, 0.40);
pub const ERROR_HIGHLIGHT_BG: Hsla = const_hsla!(0.0 / 360.0, 0.84, 0.50, 0.25);

// --- Diagnostics & Status Badges ---
pub const STATUS_SUCCESS_TEXT: Hsla = const_hsla!(142.0 / 360.0, 0.76, 0.58, 1.0);
pub const STATUS_SUCCESS_BG: Hsla = const_hsla!(142.0 / 360.0, 0.76, 0.20, 0.35);
pub const STATUS_SUCCESS_BORDER: Hsla = const_hsla!(142.0 / 360.0, 0.76, 0.40, 0.5);

pub const STATUS_ERROR_TEXT: Hsla = const_hsla!(0.0 / 360.0, 0.84, 0.68, 1.0);
pub const STATUS_ERROR_BG: Hsla = const_hsla!(0.0 / 360.0, 0.84, 0.20, 0.35);
pub const STATUS_ERROR_BORDER: Hsla = const_hsla!(0.0 / 360.0, 0.84, 0.45, 0.5);

pub const STATUS_INFO_TEXT: Hsla = const_hsla!(217.0 / 360.0, 0.91, 0.68, 1.0);
pub const STATUS_INFO_BG: Hsla = const_hsla!(217.0 / 360.0, 0.91, 0.20, 0.35);
pub const STATUS_INFO_BORDER: Hsla = const_hsla!(217.0 / 360.0, 0.91, 0.45, 0.5);

// --- Layout & Sizing Tokens ---
pub const TOOLBAR_HEIGHT: Pixels = px(44.0);
pub const STATUS_BAR_HEIGHT: Pixels = px(28.0);
pub const EDITOR_LINE_HEIGHT: Pixels = px(22.0);
pub const GUTTER_WIDTH: Pixels = px(52.0);
pub const TREE_PANEL_WIDTH: Pixels = px(320.0);
pub const CODE_PADDING_LEFT: Pixels = px(12.0);
pub const CHAR_WIDTH: f32 = 7.2;
pub const DEFAULT_WRAP_COLUMN: usize = 100;
pub const RADIUS_SM: Pixels = px(4.0);
pub const RADIUS_MD: Pixels = px(6.0);
pub const RADIUS_LG: Pixels = px(8.0);
