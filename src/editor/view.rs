//! Text editor rendering component with line numbering, virtualized scrolling, and syntax highlighting.

use super::buffer::TextSnapshot;
use super::wrap::VisualRow;
use crate::formats::json::tokenize_json_line;
use crate::theme::{
    BORDER_SUBTLE, BTN_BG_HOVER, BTN_BG_NORMAL, EDITOR_LINE_HEIGHT, ERROR_HIGHLIGHT_BG,
    GUTTER_WIDTH, LINE_ACTIVE_BG, RADIUS_SM, SELECTION_BG, STATUS_ERROR_TEXT, TEXT_MUTED,
    TEXT_PRIMARY, TEXT_SECONDARY,
};
use gpui::prelude::FluentBuilder;
use gpui::*;
use std::sync::Arc;

/// Parameters for rendering the editor view.
pub struct EditorProps<'a> {
    pub snapshot: TextSnapshot,
    pub visual_rows: Arc<[VisualRow]>,
    pub scroll_handle: UniformListScrollHandle,
    pub focus_handle: FocusHandle,
    pub cursor_offset: usize,
    pub cursor_line: usize,
    pub cursor_col: usize,
    pub selection: Option<(usize, usize)>,
    pub error_line: Option<usize>,
    pub is_search_open: bool,
    pub search_query: &'a str,
    pub match_count: usize,
    pub current_match_idx: usize,
}

/// Renders the complete editor view.
#[allow(clippy::too_many_arguments)]
pub fn render_editor<V: 'static>(
    props: EditorProps,
    cx: &mut Context<V>,
    on_row_mouse_down: impl Fn(usize, &MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    on_row_mouse_move: impl Fn(usize, &MouseMoveEvent, &mut Window, &mut App) + 'static + Clone,
    on_row_mouse_up: impl Fn(usize, &MouseUpEvent, &mut Window, &mut App) + 'static + Clone,
    on_row_click: impl Fn(usize, &ClickEvent, &mut Window, &mut App) + 'static + Clone,
    on_search_change: impl Fn(&mut V, String, &mut Window, &mut Context<V>) + 'static + Copy,
    on_search_next: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_search_prev: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_search_close: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
) -> impl IntoElement {
    let row_count = props.visual_rows.len().max(1);
    let cursor_offset = props.cursor_offset;
    let active_selection = props.selection;
    let error_line = props.error_line;
    let focus_handle = props.focus_handle.clone();
    let container_mouse_up = on_row_mouse_up.clone();

    div()
        .id("editor_panel")
        .size_full()
        .bg(crate::theme::BG_APP)
        .flex()
        .flex_col()
        .on_click(move |_event, window, _cx| {
            focus_handle.focus(window);
        })
        .on_mouse_up(MouseButton::Left, move |event, window, cx| {
            container_mouse_up(0, event, window, cx);
        })
        // Optional Search Bar at top
        .when(props.is_search_open, |this| {
            this.child(render_search_bar(
                props.search_query,
                props.match_count,
                props.current_match_idx,
                cx,
                on_search_change,
                on_search_next,
                on_search_prev,
                on_search_close,
            ))
        })
        // Editor Lines Container
        .child(
            div()
                .id("editor_scroll_container")
                .flex_1()
                .min_h(px(0.0))
                .w_full()
                .overflow_hidden()
                .child(
                    uniform_list("editor_uniform_list", row_count, {
                        let snapshot = props.snapshot.clone();
                        let visual_rows = props.visual_rows.clone();
                        let on_row_mouse_down = on_row_mouse_down.clone();
                        let on_row_mouse_move = on_row_mouse_move.clone();
                        let on_row_mouse_up = on_row_mouse_up.clone();
                        let on_row_click = on_row_click.clone();
                        move |range, _window, _cx| {
                            let snap_ref = &snapshot;
                            let rows_ref = &visual_rows;
                            range
                                .map(|row_idx| {
                                    if let Some(row) = rows_ref.get(row_idx) {
                                        render_editor_row(
                                            snap_ref,
                                            row,
                                            row_idx,
                                            cursor_offset,
                                            active_selection,
                                            error_line,
                                            on_row_mouse_down.clone(),
                                            on_row_mouse_move.clone(),
                                            on_row_mouse_up.clone(),
                                            on_row_click.clone(),
                                        )
                                    } else {
                                        render_empty_editor_row(
                                            cursor_offset == 0,
                                            on_row_mouse_down.clone(),
                                            on_row_mouse_up.clone(),
                                            on_row_click.clone(),
                                        )
                                    }
                                })
                                .collect()
                        }
                    })
                    .size_full()
                    .track_scroll(props.scroll_handle),
                ),
        )
}

#[allow(clippy::too_many_arguments)]
fn render_editor_row(
    snapshot: &TextSnapshot,
    row: &VisualRow,
    row_idx: usize,
    cursor_offset: usize,
    selection: Option<(usize, usize)>,
    error_line: Option<usize>,
    on_row_mouse_down: impl Fn(usize, &MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    on_row_mouse_move: impl Fn(usize, &MouseMoveEvent, &mut Window, &mut App) + 'static + Clone,
    on_row_mouse_up: impl Fn(usize, &MouseUpEvent, &mut Window, &mut App) + 'static + Clone,
    on_row_click: impl Fn(usize, &ClickEvent, &mut Window, &mut App) + 'static + Clone,
) -> AnyElement {
    let line_num = row.logical_line + 1;
    let is_error_line = error_line == Some(line_num);
    let full_text = snapshot.text();
    let content = if row.start_byte < full_text.len()
        && row.end_byte <= full_text.len()
        && row.start_byte <= row.end_byte
    {
        &full_text[row.start_byte..row.end_byte]
    } else {
        ""
    };

    let line_content_len = snapshot.line_content(row.logical_line).len();
    let line_start_offset = snapshot.line_start_offset(row.logical_line);
    let line_end_offset = line_start_offset + line_content_len;

    // Check if cursor falls in this visual row
    let is_cursor_in_row = cursor_offset >= row.start_byte
        && (cursor_offset < row.end_byte
            || (cursor_offset == row.end_byte
                && (row.end_byte == line_end_offset || cursor_offset == full_text.len())));

    // Compute whether this visual row is within selection
    let is_selected_row = match selection {
        Some((start, end)) => start < row.end_byte && end > row.start_byte,
        None => false,
    };

    let mut line_row = div()
        .id(ElementId::NamedInteger("ed_row".into(), row_idx as u64))
        .h(EDITOR_LINE_HEIGHT)
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .font_family("Consolas")
        .text_xs()
        .cursor_text()
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            on_row_mouse_down(row_idx, event, window, cx);
        })
        .on_mouse_move(move |event, window, cx| {
            on_row_mouse_move(row_idx, event, window, cx);
        })
        .on_mouse_up(MouseButton::Left, move |event, window, cx| {
            on_row_mouse_up(row_idx, event, window, cx);
        })
        .on_click(move |event, window, cx| {
            on_row_click(row_idx, event, window, cx);
        });

    if is_error_line {
        line_row = line_row.bg(ERROR_HIGHLIGHT_BG);
    } else if is_selected_row {
        line_row = line_row.bg(SELECTION_BG);
    } else if is_cursor_in_row {
        line_row = line_row.bg(LINE_ACTIVE_BG);
    }

    // Gutter: line number or continuation mark
    let (gutter_str, gutter_text_color) = if row.sub_line == 0 {
        let text = if is_error_line {
            format!("! {:3}", line_num)
        } else {
            format!("{:4}", line_num)
        };
        let color = if is_error_line {
            STATUS_ERROR_TEXT
        } else if is_cursor_in_row {
            TEXT_PRIMARY
        } else {
            TEXT_MUTED
        };
        (text, color)
    } else {
        ("   \u{00B7}".to_string(), TEXT_MUTED)
    };

    let gutter = div()
        .w(GUTTER_WIDTH)
        .h_full()
        .px_2()
        .bg(crate::theme::BG_GUTTER)
        .border_r_1()
        .border_color(BORDER_SUBTLE)
        .flex()
        .items_center()
        .justify_end()
        .text_color(gutter_text_color)
        .child(gutter_str);

    // Code Content with Syntax Highlighting for this visual slice
    let tokens = tokenize_json_line(content);
    let mut code_area = div()
        .flex_1()
        .h_full()
        .px_3()
        .flex()
        .flex_row()
        .items_center();

    if tokens.is_empty() {
        if is_cursor_in_row {
            code_area = code_area.child(render_cursor_caret());
        }
    } else {
        let cursor_char_in_row = if is_cursor_in_row {
            let offset_in_row = cursor_offset.saturating_sub(row.start_byte);
            let safe_slice = &content[..offset_in_row.min(content.len())];
            safe_slice.chars().count() + 1
        } else {
            0
        };

        let mut char_count_acc = 0;
        for token in tokens {
            let token_len = token.text.chars().count();
            let token_start = char_count_acc;
            let token_end = token_start + token_len;
            char_count_acc = token_end;

            if is_cursor_in_row
                && cursor_char_in_row > token_start
                && cursor_char_in_row <= token_end
            {
                let col_offset = cursor_char_in_row - 1 - token_start;
                let before: String = token.text.chars().take(col_offset).collect();
                let after: String = token.text.chars().skip(col_offset).collect();

                code_area = code_area
                    .child(div().text_color(token.color).child(before))
                    .child(render_cursor_caret())
                    .child(div().text_color(token.color).child(after));
            } else {
                code_area = code_area.child(div().text_color(token.color).child(token.text));
            }
        }

        if is_cursor_in_row && cursor_char_in_row > char_count_acc {
            code_area = code_area.child(render_cursor_caret());
        }
    }

    line_row.child(gutter).child(code_area).into_any_element()
}

fn render_empty_editor_row(
    show_caret: bool,
    on_row_mouse_down: impl Fn(usize, &MouseDownEvent, &mut Window, &mut App) + 'static + Clone,
    on_row_mouse_up: impl Fn(usize, &MouseUpEvent, &mut Window, &mut App) + 'static + Clone,
    on_row_click: impl Fn(usize, &ClickEvent, &mut Window, &mut App) + 'static + Clone,
) -> AnyElement {
    let mut code_area = div()
        .flex_1()
        .h_full()
        .px_3()
        .flex()
        .flex_row()
        .items_center();

    if show_caret {
        code_area = code_area.child(render_cursor_caret());
    }

    div()
        .id(ElementId::NamedInteger("ed_row".into(), 0))
        .h(EDITOR_LINE_HEIGHT)
        .w_full()
        .flex()
        .flex_row()
        .items_center()
        .font_family("Consolas")
        .text_xs()
        .cursor_text()
        .on_mouse_down(MouseButton::Left, move |event, window, cx| {
            on_row_mouse_down(0, event, window, cx);
        })
        .on_mouse_up(MouseButton::Left, move |event, window, cx| {
            on_row_mouse_up(0, event, window, cx);
        })
        .on_click(move |event, window, cx| {
            on_row_click(0, event, window, cx);
        })
        .child(
            div()
                .w(GUTTER_WIDTH)
                .h_full()
                .px_2()
                .bg(crate::theme::BG_GUTTER)
                .border_r_1()
                .border_color(BORDER_SUBTLE)
                .flex()
                .items_center()
                .justify_end()
                .text_color(TEXT_MUTED)
                .child("   1"),
        )
        .child(code_area)
        .into_any_element()
}

fn render_cursor_caret() -> AnyElement {
    div()
        .w(px(2.0))
        .h(px(14.0))
        .bg(TEXT_PRIMARY)
        .into_any_element()
}

#[allow(clippy::too_many_arguments)]
fn render_search_bar<V: 'static>(
    query: &str,
    match_count: usize,
    current_idx: usize,
    cx: &mut Context<V>,
    _on_change: impl Fn(&mut V, String, &mut Window, &mut Context<V>) + 'static + Copy,
    on_next: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_prev: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_close: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
) -> AnyElement {
    let match_display = if query.is_empty() {
        "No query".to_string()
    } else if match_count == 0 {
        "No matches".to_string()
    } else {
        format!("{} of {}", current_idx + 1, match_count)
    };

    div()
        .w_full()
        .px_3()
        .py_1p5()
        .bg(crate::theme::BG_PANEL_HEADER)
        .border_b_1()
        .border_color(BORDER_SUBTLE)
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
                        .text_xs()
                        .font_weight(FontWeight::MEDIUM)
                        .text_color(TEXT_MUTED)
                        .child("Find:"),
                )
                .child(
                    div()
                        .px_2()
                        .py_1()
                        .bg(crate::theme::BG_APP)
                        .border_1()
                        .border_color(crate::theme::BORDER_FOCUS)
                        .rounded(RADIUS_SM)
                        .text_xs()
                        .text_color(if query.is_empty() {
                            TEXT_MUTED
                        } else {
                            TEXT_PRIMARY
                        })
                        .min_w(px(200.0))
                        .flex()
                        .flex_row()
                        .items_center()
                        .child(if query.is_empty() {
                            "Type to search (Enter=Next)...".to_string()
                        } else {
                            query.to_string()
                        })
                        .child(
                            div()
                                .w(px(2.0))
                                .h(px(12.0))
                                .bg(crate::theme::TEXT_ACCENT)
                                .ml(px(2.0)),
                        ),
                )
                .child(div().text_xs().text_color(TEXT_MUTED).child(match_display)),
        )
        .child(
            div()
                .flex()
                .flex_row()
                .gap_1()
                .child(
                    div()
                        .id("search_btn_prev")
                        .px_2()
                        .py_1()
                        .bg(BTN_BG_NORMAL)
                        .text_color(TEXT_SECONDARY)
                        .rounded(RADIUS_SM)
                        .text_xs()
                        .cursor_pointer()
                        .hover(|s| s.bg(BTN_BG_HOVER).text_color(TEXT_PRIMARY))
                        .child("Prev")
                        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                            on_prev(this, event, window, cx);
                        })),
                )
                .child(
                    div()
                        .id("search_btn_next")
                        .px_2()
                        .py_1()
                        .bg(BTN_BG_NORMAL)
                        .text_color(TEXT_SECONDARY)
                        .rounded(RADIUS_SM)
                        .text_xs()
                        .cursor_pointer()
                        .hover(|s| s.bg(BTN_BG_HOVER).text_color(TEXT_PRIMARY))
                        .child("Next")
                        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                            on_next(this, event, window, cx);
                        })),
                )
                .child(
                    div()
                        .id("search_btn_close")
                        .px_2()
                        .py_1()
                        .bg(BTN_BG_NORMAL)
                        .text_color(TEXT_MUTED)
                        .rounded(RADIUS_SM)
                        .text_xs()
                        .cursor_pointer()
                        .hover(|s| s.bg(BTN_BG_HOVER).text_color(TEXT_PRIMARY))
                        .child("Close")
                        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                            on_close(this, event, window, cx);
                        })),
                ),
        )
        .into_any_element()
}
