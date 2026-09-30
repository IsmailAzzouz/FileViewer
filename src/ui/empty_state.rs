//! Empty state view when no document or content is loaded.
//!
//! Provides quick start actions to load files, samples, or paste text.

use crate::theme::{
    BORDER_SUBTLE, BTN_BG_HOVER, BTN_BG_NORMAL, BTN_PRIMARY_BG, BTN_PRIMARY_HOVER,
    BTN_PRIMARY_TEXT, RADIUS_MD, RADIUS_SM, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY,
};
use gpui::*;

/// Renders the empty state placeholder.
pub fn render_empty_state<V: 'static>(
    cx: &mut Context<V>,
    on_open: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_sample: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_paste: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
) -> impl IntoElement {
    div()
        .size_full()
        .flex()
        .flex_col()
        .items_center()
        .justify_center()
        .gap_4()
        .p_8()
        .child(
            div()
                .text_lg()
                .font_weight(FontWeight::SEMIBOLD)
                .text_color(TEXT_PRIMARY)
                .child("JSON Viewer & Editor"),
        )
        .child(
            div()
                .text_sm()
                .text_color(TEXT_MUTED)
                .child("Open a JSON file, load sample data, or start typing below"),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_3()
                .mt_2()
                .child(
                    div()
                        .id("empty_btn_open")
                        .px_4()
                        .py_2()
                        .bg(BTN_PRIMARY_BG)
                        .text_color(BTN_PRIMARY_TEXT)
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .rounded(RADIUS_MD)
                        .cursor_pointer()
                        .hover(|s| s.bg(BTN_PRIMARY_HOVER))
                        .child("Open File (Ctrl+O)")
                        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                            on_open(this, event, window, cx);
                        })),
                )
                .child(
                    div()
                        .id("empty_btn_sample")
                        .px_4()
                        .py_2()
                        .bg(BTN_BG_NORMAL)
                        .text_color(TEXT_SECONDARY)
                        .border_1()
                        .border_color(BORDER_SUBTLE)
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .rounded(RADIUS_MD)
                        .cursor_pointer()
                        .hover(|s| s.bg(BTN_BG_HOVER).text_color(TEXT_PRIMARY))
                        .child("Load Sample")
                        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                            on_sample(this, event, window, cx);
                        })),
                )
                .child(
                    div()
                        .id("empty_btn_paste")
                        .px_4()
                        .py_2()
                        .bg(BTN_BG_NORMAL)
                        .text_color(TEXT_SECONDARY)
                        .border_1()
                        .border_color(BORDER_SUBTLE)
                        .text_sm()
                        .font_weight(FontWeight::MEDIUM)
                        .rounded(RADIUS_MD)
                        .cursor_pointer()
                        .hover(|s| s.bg(BTN_BG_HOVER).text_color(TEXT_PRIMARY))
                        .child("Paste Clipboard (Ctrl+V)")
                        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                            on_paste(this, event, window, cx);
                        })),
                ),
        )
        .child(
            div()
                .mt_4()
                .p_3()
                .bg(BTN_BG_NORMAL)
                .border_1()
                .border_color(BORDER_SUBTLE)
                .rounded(RADIUS_SM)
                .text_xs()
                .text_color(TEXT_MUTED)
                .child("Shortcuts: Ctrl+O Open | Ctrl+S Save | Ctrl+Shift+F Format | Ctrl+F Find"),
        )
}
