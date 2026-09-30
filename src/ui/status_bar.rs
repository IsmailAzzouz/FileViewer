//! Status bar displaying document metadata, cursor position, and validation status.

use crate::formats::json::JsonDiagnostic;
use crate::theme::{
    BORDER_SUBTLE, RADIUS_SM, STATUS_BAR_HEIGHT, STATUS_ERROR_BG, STATUS_ERROR_BORDER,
    STATUS_ERROR_TEXT, STATUS_SUCCESS_BG, STATUS_SUCCESS_BORDER, STATUS_SUCCESS_TEXT, TEXT_MUTED,
    TEXT_PRIMARY, TEXT_SECONDARY,
};
use gpui::prelude::FluentBuilder;
use gpui::*;

/// Parameters for rendering the status bar.
pub struct StatusBarProps<'a> {
    pub file_path: Option<&'a str>,
    pub is_dirty: bool,
    pub status_message: Option<&'a str>,
    pub line: usize,
    pub col: usize,
    pub line_count: usize,
    pub char_count: usize,
    pub selection_len: Option<usize>,
    pub diagnostic: Option<&'a JsonDiagnostic>,
    pub node_count: Option<usize>,
}

/// Renders the bottom status bar.
pub fn render_status_bar<V: 'static>(
    props: StatusBarProps,
    cx: &mut Context<V>,
    on_error_click: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
) -> impl IntoElement {
    let file_display = match props.file_path {
        Some(path) => {
            let file_name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(path);
            if props.is_dirty {
                format!("* {}", file_name)
            } else {
                file_name.to_string()
            }
        }
        None => {
            if props.is_dirty {
                "* Untitled.json".to_string()
            } else {
                "Untitled.json".to_string()
            }
        }
    };

    let selection_text = match props.selection_len {
        Some(len) if len > 0 => format!(" ({} sel)", len),
        _ => String::new(),
    };

    div()
        .w_full()
        .h(STATUS_BAR_HEIGHT)
        .px_3()
        .bg(crate::theme::BG_STATUS_BAR)
        .border_t_1()
        .border_color(BORDER_SUBTLE)
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        .text_xs()
        // Left section: validation pill, file name, and status message
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_3()
                .child(render_validation_badge(
                    props.char_count == 0,
                    props.diagnostic,
                    props.node_count,
                    cx,
                    on_error_click,
                ))
                .child(div().text_color(TEXT_SECONDARY).child(file_display))
                .when_some(props.status_message, |this, msg| {
                    this.child(
                        div()
                            .px_2()
                            .py_0p5()
                            .bg(crate::theme::STATUS_INFO_BG)
                            .border_1()
                            .border_color(crate::theme::STATUS_INFO_BORDER)
                            .text_color(crate::theme::STATUS_INFO_TEXT)
                            .rounded(RADIUS_SM)
                            .child(msg.to_string()),
                    )
                }),
        )
        // Right section: line/col, document stats, format
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_4()
                .text_color(TEXT_MUTED)
                .child(div().child(format!(
                    "Ln {}, Col {}{}",
                    props.line, props.col, selection_text
                )))
                .child(div().child(format!(
                    "{} lines, {} chars",
                    props.line_count, props.char_count
                )))
                .child(
                    div()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(TEXT_PRIMARY)
                        .child("JSON (UTF-8)"),
                ),
        )
}

fn render_validation_badge<V: 'static>(
    is_empty: bool,
    diagnostic: Option<&JsonDiagnostic>,
    node_count: Option<usize>,
    cx: &mut Context<V>,
    on_error_click: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
) -> AnyElement {
    if is_empty {
        div()
            .px_2()
            .py_0p5()
            .bg(BORDER_SUBTLE)
            .text_color(TEXT_MUTED)
            .rounded(RADIUS_SM)
            .child("Empty")
            .into_any_element()
    } else if let Some(err) = diagnostic {
        div()
            .id("status_error_badge")
            .px_2()
            .py_0p5()
            .bg(STATUS_ERROR_BG)
            .border_1()
            .border_color(STATUS_ERROR_BORDER)
            .text_color(STATUS_ERROR_TEXT)
            .rounded(RADIUS_SM)
            .cursor_pointer()
            .child(format!("Invalid: Ln {}, Col {}", err.line, err.column))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                on_error_click(this, event, window, cx);
            }))
            .into_any_element()
    } else {
        let count_str = node_count
            .map(|c| format!(" ({} nodes)", c))
            .unwrap_or_default();
        div()
            .px_2()
            .py_0p5()
            .bg(STATUS_SUCCESS_BG)
            .border_1()
            .border_color(STATUS_SUCCESS_BORDER)
            .text_color(STATUS_SUCCESS_TEXT)
            .rounded(RADIUS_SM)
            .child(format!("Valid JSON{}", count_str))
            .into_any_element()
    }
}
