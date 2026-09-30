//! Application toolbar containing document actions and view toggles.

use crate::theme::{
    BORDER_SUBTLE, BTN_BG_ACTIVE, BTN_BG_HOVER, BTN_BG_NORMAL, BTN_PRIMARY_BG, BTN_PRIMARY_HOVER,
    BTN_PRIMARY_TEXT, RADIUS_MD, TEXT_MUTED, TEXT_PRIMARY, TEXT_SECONDARY, TOOLBAR_HEIGHT,
};
use gpui::*;

/// Parameters for rendering the top action toolbar.
pub struct ToolbarProps {
    pub is_dirty: bool,
    pub is_tree_visible: bool,
    pub is_search_open: bool,
    pub is_word_wrap: bool,
    pub has_content: bool,
    pub is_busy: bool,
}

/// Renders the top toolbar.
#[allow(clippy::too_many_arguments)]
pub fn render_toolbar<V: 'static>(
    props: ToolbarProps,
    cx: &mut Context<V>,
    on_open: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_save: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_format: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_minify: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_clear: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_copy: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_sample: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_toggle_search: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_toggle_wrap: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
    on_toggle_tree: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
) -> impl IntoElement {
    div()
        .w_full()
        .h(TOOLBAR_HEIGHT)
        .px_3()
        .bg(crate::theme::BG_TOOLBAR)
        .border_b_1()
        .border_color(BORDER_SUBTLE)
        .flex()
        .flex_row()
        .items_center()
        .justify_between()
        // Left actions: File operations
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(render_tool_btn(
                    "tb_open",
                    "Open",
                    Some("Ctrl+O"),
                    false,
                    props.is_busy,
                    cx,
                    on_open,
                ))
                .child(render_tool_btn(
                    "tb_save",
                    "Save",
                    Some("Ctrl+S"),
                    false,
                    props.is_busy || !props.is_dirty,
                    cx,
                    on_save,
                ))
                .child(div().w(px(1.0)).h(px(18.0)).bg(BORDER_SUBTLE).mx_1())
                .child(render_tool_btn(
                    "tb_format",
                    "Format",
                    Some("Ctrl+Shift+F"),
                    true,
                    props.is_busy || !props.has_content,
                    cx,
                    on_format,
                ))
                .child(render_tool_btn(
                    "tb_minify",
                    "Minify",
                    None,
                    false,
                    props.is_busy || !props.has_content,
                    cx,
                    on_minify,
                ))
                .child(div().w(px(1.0)).h(px(18.0)).bg(BORDER_SUBTLE).mx_1())
                .child(render_tool_btn(
                    "tb_copy",
                    "Copy All",
                    None,
                    false,
                    props.is_busy || !props.has_content,
                    cx,
                    on_copy,
                ))
                .child(render_tool_btn(
                    "tb_clear",
                    "Clear",
                    None,
                    false,
                    props.is_busy || !props.has_content,
                    cx,
                    on_clear,
                ))
                .child(render_tool_btn(
                    "tb_sample",
                    "Sample",
                    None,
                    false,
                    props.is_busy,
                    cx,
                    on_sample,
                )),
        )
        // Right actions: View toggles & search
        .child(
            div()
                .flex()
                .flex_row()
                .items_center()
                .gap_2()
                .child(render_toggle_btn(
                    "tb_search",
                    "Find",
                    Some("Ctrl+F"),
                    props.is_search_open,
                    cx,
                    on_toggle_search,
                ))
                .child(render_toggle_btn(
                    "tb_wrap",
                    "Wrap",
                    Some("Alt+Z"),
                    props.is_word_wrap,
                    cx,
                    on_toggle_wrap,
                ))
                .child(render_toggle_btn(
                    "tb_tree",
                    "Tree View",
                    None,
                    props.is_tree_visible,
                    cx,
                    on_toggle_tree,
                )),
        )
}

fn render_tool_btn<V: 'static>(
    id: &'static str,
    label: &'static str,
    shortcut: Option<&'static str>,
    is_primary: bool,
    disabled: bool,
    cx: &mut Context<V>,
    handler: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
) -> AnyElement {
    let (bg, hover_bg, text_color) = if is_primary {
        (BTN_PRIMARY_BG, BTN_PRIMARY_HOVER, BTN_PRIMARY_TEXT)
    } else {
        (BTN_BG_NORMAL, BTN_BG_HOVER, TEXT_SECONDARY)
    };

    let mut element = div()
        .id(id)
        .px_3()
        .py_1p5()
        .bg(bg)
        .text_color(text_color)
        .rounded(RADIUS_MD)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .flex()
        .flex_row()
        .items_center()
        .gap_1p5();

    if disabled {
        element = element.opacity(0.4).cursor_not_allowed();
    } else {
        element = element
            .cursor_pointer()
            .hover(move |s| s.bg(hover_bg).text_color(TEXT_PRIMARY))
            .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
                handler(this, event, window, cx);
            }));
    }

    element = element.child(label);

    if let Some(sc) = shortcut {
        element = element.child(
            div()
                .text_color(if is_primary {
                    hsla(0.0, 0.0, 1.0, 0.7)
                } else {
                    TEXT_MUTED
                })
                .child(format!("({})", sc)),
        );
    }

    element.into_any_element()
}

fn render_toggle_btn<V: 'static>(
    id: &'static str,
    label: &'static str,
    shortcut: Option<&'static str>,
    is_active: bool,
    cx: &mut Context<V>,
    handler: impl Fn(&mut V, &ClickEvent, &mut Window, &mut Context<V>) + 'static + Copy,
) -> AnyElement {
    let bg = if is_active {
        BTN_BG_ACTIVE
    } else {
        BTN_BG_NORMAL
    };
    let border = if is_active {
        crate::theme::BORDER_FOCUS
    } else {
        BORDER_SUBTLE
    };
    let text = if is_active {
        TEXT_PRIMARY
    } else {
        TEXT_SECONDARY
    };

    let mut element = div()
        .id(id)
        .px_3()
        .py_1p5()
        .bg(bg)
        .border_1()
        .border_color(border)
        .text_color(text)
        .rounded(RADIUS_MD)
        .text_xs()
        .font_weight(FontWeight::MEDIUM)
        .cursor_pointer()
        .flex()
        .flex_row()
        .items_center()
        .gap_1p5()
        .hover(|s| s.bg(BTN_BG_HOVER).text_color(TEXT_PRIMARY))
        .on_click(cx.listener(move |this, event: &ClickEvent, window, cx| {
            handler(this, event, window, cx);
        }))
        .child(label);

    if let Some(sc) = shortcut {
        element = element.child(div().text_color(TEXT_MUTED).child(format!("({})", sc)));
    }

    element.into_any_element()
}
