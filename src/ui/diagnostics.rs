//! Diagnostics banner displaying JSON validation errors and snippets.

use crate::formats::json::JsonDiagnostic;
use crate::theme::{
    BTN_BG_HOVER, BTN_PRIMARY_BG, BTN_PRIMARY_HOVER, BTN_PRIMARY_TEXT, RADIUS_MD, RADIUS_SM,
    STATUS_ERROR_BG, STATUS_ERROR_BORDER, STATUS_ERROR_TEXT, TEXT_PRIMARY,
};
use gpui::*;

/// Renders a diagnostic banner when a JSON parse error is present.
pub fn render_diagnostics<V: 'static>(
    diagnostic: &JsonDiagnostic,
    cx: &mut Context<V>,
    on_jump: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_dismiss: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
) -> impl IntoElement {
    div()
        .w_full()
        .p_3()
        .bg(STATUS_ERROR_BG)
        .border_b_1()
        .border_color(STATUS_ERROR_BORDER)
        .flex()
        .flex_col()
        .gap_2()
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .justify_between()
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_2()
                        .child(
                            div()
                                .font_weight(FontWeight::BOLD)
                                .text_color(STATUS_ERROR_TEXT)
                                .text_sm()
                                .child(format!(
                                    "JSON Syntax Error (Line {}, Col {})",
                                    diagnostic.line, diagnostic.column
                                )),
                        )
                        .child(
                            div()
                                .text_xs()
                                .text_color(TEXT_PRIMARY)
                                .child(diagnostic.message.clone()),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .gap_2()
                        .child(
                            div()
                                .id("diag_jump_btn")
                                .px_3()
                                .py_1()
                                .bg(BTN_PRIMARY_BG)
                                .text_color(BTN_PRIMARY_TEXT)
                                .text_xs()
                                .font_weight(FontWeight::MEDIUM)
                                .rounded(RADIUS_SM)
                                .cursor_pointer()
                                .hover(|s| s.bg(BTN_PRIMARY_HOVER))
                                .child("Jump to Line")
                                .on_click(cx.listener(
                                    move |this, event: &ClickEvent, window, cx| {
                                        on_jump(this, event, window, cx);
                                    },
                                )),
                        )
                        .child(
                            div()
                                .id("diag_dismiss_btn")
                                .px_2()
                                .py_1()
                                .bg(STATUS_ERROR_BORDER)
                                .text_color(TEXT_PRIMARY)
                                .text_xs()
                                .rounded(RADIUS_SM)
                                .cursor_pointer()
                                .hover(|s| s.bg(BTN_BG_HOVER))
                                .child("Dismiss")
                                .on_click(cx.listener(
                                    move |this, event: &ClickEvent, window, cx| {
                                        on_dismiss(this, event, window, cx);
                                    },
                                )),
                        ),
                ),
        )
        .child(
            div()
                .p_2()
                .bg(hsla(0.0, 0.0, 0.05, 0.7))
                .rounded(RADIUS_MD)
                .font_family("Consolas")
                .text_xs()
                .child(
                    div()
                        .text_color(TEXT_PRIMARY)
                        .child(diagnostic.context_snippet.clone()),
                )
                .child(
                    div()
                        .text_color(STATUS_ERROR_TEXT)
                        .font_weight(FontWeight::BOLD)
                        .child(diagnostic.pointer.clone()),
                ),
        )
}
