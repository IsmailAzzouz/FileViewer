//! Tree view component for rendering the JSON hierarchical structure.

use super::{TreeRowData, TreeState};
use crate::formats::json::{JsonSpan, JsonType};
use crate::theme::{
    BORDER_SUBTLE, BTN_BG_HOVER, BTN_BG_NORMAL, RADIUS_SM, SELECTION_BG, SYNTAX_BOOLEAN,
    SYNTAX_KEY, SYNTAX_NULL, SYNTAX_NUMBER, SYNTAX_PUNCTUATION, SYNTAX_STRING, TEXT_MUTED,
    TEXT_PRIMARY,
};
use gpui::prelude::FluentBuilder;
use gpui::*;

/// Renders the JSON Tree panel with O(1) cached visible rows.
#[allow(clippy::too_many_arguments)]
pub fn render_tree_view<V: 'static>(
    tree: &TreeState,
    cx: &mut Context<V>,
    on_toggle_expand: impl Fn(String, &ClickEvent, &mut Window, &mut App) + 'static + Clone,
    on_select_node: impl Fn(usize, JsonSpan, &ClickEvent, &mut Window, &mut App) + 'static + Clone,
    on_expand_all: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_collapse_all: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_prev_symbol: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_next_symbol: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
) -> impl IntoElement {
    let total_nodes = tree.total_node_count();
    let selected_id = tree.selected_id();
    let rows = tree.cached_rows();
    let node_count = rows.len();

    div()
        .w(px(320.0))
        .h_full()
        .bg(crate::theme::BG_PANEL)
        .border_r_1()
        .border_color(BORDER_SUBTLE)
        .flex()
        .flex_col()
        // Header
        .child(
            div()
                .h(px(40.0))
                .px_3()
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
                                .font_weight(FontWeight::SEMIBOLD)
                                .text_xs()
                                .text_color(TEXT_PRIMARY)
                                .child("JSON Tree"),
                        )
                        .child(
                            div()
                                .px_1p5()
                                .py_0p5()
                                .bg(BORDER_SUBTLE)
                                .text_color(TEXT_MUTED)
                                .rounded(RADIUS_SM)
                                .text_xs()
                                .child(format!("{} nodes", total_nodes)),
                        ),
                )
                .child(
                    div()
                        .flex()
                        .flex_row()
                        .items_center()
                        .gap_1()
                        .child(
                            div()
                                .id("tree_btn_prev_symbol")
                                .px_1p5()
                                .py_1()
                                .bg(BTN_BG_NORMAL)
                                .text_color(TEXT_MUTED)
                                .rounded(RADIUS_SM)
                                .text_xs()
                                .cursor_pointer()
                                .hover(|s| s.bg(BTN_BG_HOVER).text_color(TEXT_PRIMARY))
                                .child("<")
                                .on_click(cx.listener(
                                    move |this, event: &ClickEvent, window, cx| {
                                        on_prev_symbol(this, event, window, cx);
                                    },
                                )),
                        )
                        .child(
                            div()
                                .id("tree_btn_next_symbol")
                                .px_1p5()
                                .py_1()
                                .bg(BTN_BG_NORMAL)
                                .text_color(TEXT_MUTED)
                                .rounded(RADIUS_SM)
                                .text_xs()
                                .cursor_pointer()
                                .hover(|s| s.bg(BTN_BG_HOVER).text_color(TEXT_PRIMARY))
                                .child(">")
                                .on_click(cx.listener(
                                    move |this, event: &ClickEvent, window, cx| {
                                        on_next_symbol(this, event, window, cx);
                                    },
                                )),
                        )
                        .child(
                            div()
                                .id("tree_btn_expand_all")
                                .px_2()
                                .py_1()
                                .bg(BTN_BG_NORMAL)
                                .text_color(TEXT_MUTED)
                                .rounded(RADIUS_SM)
                                .text_xs()
                                .cursor_pointer()
                                .hover(|s| s.bg(BTN_BG_HOVER).text_color(TEXT_PRIMARY))
                                .child("Expand")
                                .on_click(cx.listener(
                                    move |this, event: &ClickEvent, window, cx| {
                                        on_expand_all(this, event, window, cx);
                                    },
                                )),
                        )
                        .child(
                            div()
                                .id("tree_btn_collapse_all")
                                .px_2()
                                .py_1()
                                .bg(BTN_BG_NORMAL)
                                .text_color(TEXT_MUTED)
                                .rounded(RADIUS_SM)
                                .text_xs()
                                .cursor_pointer()
                                .hover(|s| s.bg(BTN_BG_HOVER).text_color(TEXT_PRIMARY))
                                .child("Collapse")
                                .on_click(cx.listener(
                                    move |this, event: &ClickEvent, window, cx| {
                                        on_collapse_all(this, event, window, cx);
                                    },
                                )),
                        ),
                ),
        )
        // Tree Node Virtualized List
        .child(
            div()
                .id("tree_node_scroll_area")
                .flex_1()
                .min_h(px(0.0))
                .w_full()
                .overflow_hidden()
                .when(node_count == 0, |this| {
                    this.child(
                        div()
                            .p_4()
                            .text_xs()
                            .text_color(TEXT_MUTED)
                            .child("No JSON tree loaded"),
                    )
                })
                .when(node_count > 0, |this| {
                    let rows = rows.clone();
                    let on_toggle_expand = on_toggle_expand.clone();
                    let on_select_node = on_select_node.clone();
                    this.child(
                        uniform_list(
                            "tree_uniform_list",
                            node_count,
                            move |range, _window, _cx| {
                                let rows = rows.clone();
                                let on_toggle_expand = on_toggle_expand.clone();
                                let on_select_node = on_select_node.clone();
                                range
                                    .map(move |idx| {
                                        let row_data = &rows[idx];
                                        let is_selected = selected_id == Some(row_data.id);
                                        render_tree_node_row(
                                            row_data,
                                            is_selected,
                                            on_toggle_expand.clone(),
                                            on_select_node.clone(),
                                        )
                                    })
                                    .collect()
                            },
                        )
                        .size_full()
                        .track_scroll(tree.scroll_handle()),
                    )
                }),
        )
}

fn render_tree_node_row(
    data: &TreeRowData,
    is_selected: bool,
    on_toggle: impl Fn(String, &ClickEvent, &mut Window, &mut App) + 'static + Clone,
    on_select: impl Fn(usize, JsonSpan, &ClickEvent, &mut Window, &mut App) + 'static + Clone,
) -> AnyElement {
    let node_id = data.id;
    let node_span = data.full_span;
    let node_path = data.path.to_string();
    let has_children = data.has_children;

    let indent_px = px((data.depth * 16) as f32);

    let on_select_click = on_select.clone();
    let mut row = div()
        .id(ElementId::NamedInteger("tree_row".into(), node_id as u64))
        .h(px(24.0))
        .w_full()
        .px_2()
        .flex()
        .flex_row()
        .items_center()
        .gap_1p5()
        .text_xs()
        .cursor_pointer()
        .hover(|s| s.bg(BTN_BG_HOVER))
        .on_click(move |event, window, cx| {
            on_select_click(node_id, node_span, event, window, cx);
        });

    if is_selected {
        row = row
            .bg(SELECTION_BG)
            .border_l_2()
            .border_color(crate::theme::BORDER_FOCUS);
    }

    // Indent spacing
    row = row.child(div().w(indent_px));

    // Chevron or leaf marker
    if has_children {
        let chevron_symbol = if data.is_expanded { "v" } else { ">" };
        let toggle_path = node_path.clone();
        let on_toggle_click = on_toggle.clone();
        row = row.child(
            div()
                .id(ElementId::NamedInteger(
                    "tree_toggle".into(),
                    node_id as u64,
                ))
                .w(px(14.0))
                .h(px(14.0))
                .flex()
                .items_center()
                .justify_center()
                .text_color(TEXT_MUTED)
                .hover(|s| s.text_color(TEXT_PRIMARY))
                .child(chevron_symbol)
                .on_click(move |event, window, cx| {
                    cx.stop_propagation();
                    on_toggle_click(toggle_path.clone(), event, window, cx);
                }),
        );
    } else {
        row = row.child(
            div()
                .w(px(14.0))
                .h(px(14.0))
                .flex()
                .items_center()
                .justify_center()
                .text_color(TEXT_MUTED)
                .child("-"),
        );
    }

    // Type Badge
    row = row.child(render_type_badge(data.json_type));

    // Key (if property of object)
    if let Some(ref k) = data.key {
        row = row.child(
            div()
                .font_weight(FontWeight::MEDIUM)
                .text_color(SYNTAX_KEY)
                .child(k.clone()),
        );
    }

    // Value preview
    let val_color = match data.json_type {
        JsonType::String => SYNTAX_STRING,
        JsonType::Number => SYNTAX_NUMBER,
        JsonType::Boolean => SYNTAX_BOOLEAN,
        JsonType::Null => SYNTAX_NULL,
        JsonType::Object | JsonType::Array => TEXT_MUTED,
    };

    row = row.child(
        div()
            .text_color(val_color)
            .child(data.value_preview.clone()),
    );

    row.into_any_element()
}

fn render_type_badge(t: JsonType) -> AnyElement {
    let (color, text) = match t {
        JsonType::Object => (SYNTAX_PUNCTUATION, "{ }"),
        JsonType::Array => (SYNTAX_PUNCTUATION, "[ ]"),
        JsonType::String => (SYNTAX_STRING, "str"),
        JsonType::Number => (SYNTAX_NUMBER, "num"),
        JsonType::Boolean => (SYNTAX_BOOLEAN, "bool"),
        JsonType::Null => (SYNTAX_NULL, "null"),
    };

    div()
        .px_1()
        .bg(BORDER_SUBTLE)
        .rounded(RADIUS_SM)
        .text_color(color)
        .font_family("Consolas")
        .text_xs()
        .child(text)
        .into_any_element()
}
