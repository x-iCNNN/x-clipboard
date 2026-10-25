use super::*;
mod settings;
mod welcome;
use gpui_kit::component::switch::Switch;

fn accent(cx: &App) -> Hsla {
    super::accent(cx)
}
#[inline(never)]
fn page_heading(title: &str, description: &str, cx: &App) -> AnyElement {
    div()
        .px(px(32.))
        .pt(px(22.))
        .pb(px(22.))
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(8.))
        .child(
            div()
                .text_size(px(20.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(tr(title).to_owned()),
        )
        .child(value(description, cx))
        .into_any_element()
}
fn section_heading(title: &str) -> AnyElement {
    div()
        .text_size(px(13.))
        .font_weight(FontWeight::SEMIBOLD)
        .child(tr(title).to_owned())
        .into_any_element()
}
#[inline(never)]
fn section(title: &str, children: Vec<AnyElement>, cx: &App) -> AnyElement {
    div()
        .flex_shrink_0()
        .flex()
        .flex_col()
        .gap(px(9.))
        .child(section_heading(title))
        .child(
            div()
                .border_1()
                .border_color(cx.theme().border)
                .rounded(px(8.))
                .overflow_hidden()
                .bg(cx.theme().background)
                .children(children),
        )
        .into_any_element()
}
#[inline(never)]
fn setting_row(
    label: &str,
    description: &str,
    control: AnyElement,
    last: bool,
    cx: &App,
) -> AnyElement {
    div()
        .min_h(px(if description.is_empty() { 52. } else { 66. }))
        .px(px(16.))
        .py(px(13.))
        .flex()
        .items_center()
        .gap(px(18.))
        .when(!last, |d| d.border_b_1().border_color(cx.theme().border))
        .child(
            div()
                .flex_1()
                .min_w_0()
                .flex()
                .flex_col()
                .gap(px(5.))
                .child(
                    div()
                        .text_size(px(14.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(tr(label).to_owned()),
                )
                .when(!description.is_empty(), |d| d.child(value(description, cx))),
        )
        .child(div().flex_shrink_0().child(control))
        .into_any_element()
}
fn value(label: &str, cx: &App) -> AnyElement {
    div()
        .text_size(px(12.))
        .text_color(cx.theme().muted_foreground)
        .child(tr(label).to_owned())
        .into_any_element()
}
fn badge(label: &str, tinted: bool, cx: &App) -> AnyElement {
    div()
        .px(px(7.))
        .py(px(3.))
        .rounded(px(5.))
        .text_size(px(11.))
        .text_color(if tinted {
            accent(cx)
        } else {
            cx.theme().muted_foreground
        })
        .bg(if tinted {
            accent(cx).opacity(0.1)
        } else {
            cx.theme().muted
        })
        .child(tr(label).to_owned())
        .into_any_element()
}
fn notice(icon: IconName, label: &str, warning: bool, cx: &App) -> AnyElement {
    let tint: Hsla = if warning {
        rgb(0xa67026).into()
    } else {
        accent(cx)
    };
    div()
        .p(px(14.))
        .flex()
        .gap(px(10.))
        .items_start()
        .border_1()
        .border_color(tint.opacity(0.18))
        .rounded(px(8.))
        .bg(tint.opacity(0.08))
        .text_color(tint)
        .text_size(px(12.))
        .child(Icon::new(icon).size(px(17.)).flex_shrink_0())
        .child(div().flex_1().min_w_0().child(tr(label).to_owned()))
        .into_any_element()
}
fn brand_icon(icon: IconName, size: f32, soft: bool, cx: &App) -> AnyElement {
    div()
        .size(px(size))
        .flex_shrink_0()
        .rounded(px(size * 0.28))
        .bg(if soft {
            accent(cx).opacity(0.1)
        } else {
            cx.theme().button_primary
        })
        .text_color(if soft {
            accent(cx)
        } else {
            rgb(0xffffff).into()
        })
        .flex()
        .items_center()
        .justify_center()
        .child(Icon::new(icon).size(px(size * 0.48)))
        .into_any_element()
}
fn feature(icon: IconName, title: &str, desc: &str, cx: &App) -> AnyElement {
    div()
        .flex()
        .items_center()
        .gap(px(13.))
        .child(Icon::new(icon).size(px(20.)).text_color(accent(cx)))
        .child(
            div()
                .flex()
                .flex_col()
                .gap(px(4.))
                .child(
                    div()
                        .text_size(px(12.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr(title).to_owned()),
                )
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(cx.theme().muted_foreground)
                        .child(tr(desc).to_owned()),
                ),
        )
        .into_any_element()
}
fn display_shortcut(shortcut: &str) -> String {
    shortcut.replace(
        "Super",
        if cfg!(target_os = "macos") {
            "⌘"
        } else {
            "Super"
        },
    )
}
#[inline(never)]
fn keys(shortcut: &str, large: bool, cx: &App) -> AnyElement {
    match shortcut_keystroke(shortcut) {
        Some(stroke) => Kbd::new(stroke)
            .text_size(px(if large { 20. } else { 11. }))
            .h(px(if large { 48. } else { 24. }))
            .into_any_element(),
        None => keycap(shortcut.to_owned(), cx),
    }
}

fn symbol_display(
    item: &x_clipboard::symbols::Symbol,
    emoji: bool,
    window: &Window,
    cx: &App,
) -> (AnyElement, bool) {
    let size = px(if emoji { 25. } else { 22. });
    let layout = window.text_system().shape_line(
        item.symbol.clone().into(),
        size,
        &[TextRun {
            len: item.symbol.len(),
            font: font(cx.theme().font_family.clone()),
            color: cx.theme().foreground,
            ..Default::default()
        }],
        None,
    );
    let supported = !layout
        .runs
        .iter()
        .flat_map(|run| &run.glyphs)
        .any(|glyph| glyph.id.0 == 0)
        && (!emoji || layout.width <= px(35.));
    let content = if supported {
        div()
            .text_size(size)
            .child(item.symbol.clone())
            .into_any_element()
    } else {
        div()
            .w(px(34.))
            .whitespace_normal()
            .text_size(px(11.))
            .line_height(px(13.))
            .text_center()
            .text_color(cx.theme().muted_foreground)
            .child(item.name.chars().take(4).collect::<String>())
            .into_any_element()
    };
    (content, supported)
}

impl Desktop {
    fn language_picker(&self, id: &'static str) -> AnyElement {
        let selected = self.snapshot.settings.language.clone();
        let service = self.service.clone();
        Button::new(id)
            .small()
            .icon(IconName::Globe)
            .label(i18n::language_name(&selected))
            .accessibility_label(tr("界面语言"))
            .dropdown_menu(move |mut menu, _, _| {
                for &(language, name) in i18n::LANGUAGES {
                    let service = service.clone();
                    menu = menu.item(PopupMenuItem::new(name)
                        .checked(language == selected)
                        .on_click(move |_, _, _| {
                            let mut settings = service.snapshot().settings;
                            settings.language = language.into();
                            service.send(Command::Settings(settings));
                        }));
                }
                menu
            })
            .into_any_element()
    }
    #[inline(never)]
    pub(super) fn symbols(&self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let query = self.search.read(cx).value().to_lowercase();
        let emoji = self.route == "emoji";
        let catalogue = if emoji {
            x_clipboard::symbols::emoji()
        } else {
            x_clipboard::symbols::punctuation()
        };
        let mut values: Vec<_> = catalogue
            .iter()
            .filter(|item| {
                (self.filter == "all"
                    || self.filter == item.group
                    || (self.filter == "recent"
                        && self.snapshot.settings.recent_symbols.contains(&item.symbol)))
                    && (query.is_empty()
                        || item.name.to_lowercase().contains(&query)
                        || item.keywords.to_lowercase().contains(&query)
                        || item.symbol.contains(&query))
            })
            .cloned()
            .collect();
        if self.filter == "recent" {
            values.sort_by_key(|item| {
                self.snapshot
                    .settings
                    .recent_symbols
                    .iter()
                    .position(|s| s == &item.symbol)
                    .unwrap_or(usize::MAX)
            });
        }
        let quick = self.kind == Kind::Quick;
        let padding = if quick { 16. } else { 32. };
        let width = f32::from(window.viewport_size().width) - if quick { 32. } else { 264. };
        let columns = ((width / 47.) as usize).max(4);
        let count = values.len();
        let categories = self.symbol_categories(emoji, padding, cx);
        let grid = self.symbol_grid(values, columns, padding, emoji, cx);
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .when(!quick, |d| {
                d.child(page_heading(
                    if emoji { "表情" } else { "符号" },
                    "离线中文搜索 · 点击插入或复制",
                    cx,
                ))
            })
            .when(quick, |d| {
                d.child(
                    div()
                        .px(px(12.))
                        .pt(px(4.))
                        .pb(px(10.))
                        .flex()
                        .items_center()
                        .gap_2()
                        .child(
                            Button::new("emoji-back")
                                .ghost()
                                .small()
                                .icon(IconName::ChevronLeft)
                                .tooltip(tr("返回历史"))
                                .accessibility_label(tr("返回历史"))
                                .on_click(cx.listener(|t, _, _, cx| t.nav("all", cx))),
                        )
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::SEMIBOLD)
                                .child(tr(if emoji { "表情" } else { "符号" })),
                        ),
                )
            })
            .child(
                div().px(px(padding)).child(
                    Input::new(&self.search).small().cleanable(true).prefix(
                        Icon::new(IconName::Search)
                            .size(px(15.))
                            .text_color(cx.theme().muted_foreground),
                    ),
                ),
            )
            .child(categories)
            .child(
                div()
                    .px(px(padding))
                    .pb(px(10.))
                    .flex()
                    .items_center()
                    .justify_between()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child(tr(if !query.is_empty() {
                        "搜索结果"
                    } else if self.filter == "recent" {
                        "最近使用"
                    } else {
                        "点击插入"
                    }))
                    .child(message("{count} 项", &[("count", count.to_string())])),
            )
            .when(count > 0, |d| d.child(grid))
            .when(count == 0, |d| {
                d.child(
                    div()
                        .flex_1()
                        .flex()
                        .flex_col()
                        .items_center()
                        .justify_center()
                        .gap_3()
                        .px_5()
                        .pb_10()
                        .child(
                            Icon::new(IconName::Search)
                                .size(px(28.))
                                .text_color(cx.theme().muted_foreground),
                        )
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(tr(if self.filter == "recent" && query.is_empty() {
                                    "还没有最近使用的内容"
                                } else {
                                    "没有找到匹配内容"
                                })),
                        )
                        .child(value(
                            if self.filter == "recent" && query.is_empty() {
                                "插入过的表情与符号会显示在这里。"
                            } else {
                                "试试其他名称、关键词或分类。"
                            },
                            cx,
                        )),
                )
            })
            .into_any_element()
    }
    #[inline(never)]
    fn symbol_categories(&self, emoji: bool, padding: f32, cx: &mut Context<Self>) -> AnyElement {
        let categories = div()
            .px(px(padding))
            .py(px(11.))
            .flex()
            .flex_wrap()
            .gap(px(3.))
            .children(
                (if emoji {
                    vec![
                        ("all", "全部"),
                        ("recent", "最近"),
                        ("表情", "表情"),
                        ("人物", "人物"),
                        ("自然", "自然"),
                        ("食物", "食物"),
                        ("旅行", "旅行"),
                        ("活动", "活动"),
                        ("物品", "物品"),
                        ("符号", "符号"),
                        ("旗帜", "旗帜"),
                    ]
                } else {
                    vec![
                        ("all", "全部"),
                        ("recent", "最近"),
                        ("标点", "标点"),
                        ("数学", "数学"),
                        ("箭头", "箭头"),
                        ("货币", "货币"),
                        ("几何", "几何"),
                    ]
                })
                .into_iter()
                .map(|(group, label)| {
                    Button::new(SharedString::from(format!("emoji-group-{group}")))
                        .ghost()
                        .small()
                        .label(tr(label).to_owned())
                        .selected(self.filter == group)
                        .when(self.filter == group, |button| {
                            button.bg(accent(cx).opacity(0.1)).text_color(accent(cx))
                        })
                        .on_click(cx.listener(move |t, _, _, cx| {
                            t.filter = group;
                            cx.notify();
                        }))
                }),
            );

        categories.into_any_element()
    }

    #[inline(never)]
    fn symbol_grid(
        &self,
        values: Vec<x_clipboard::symbols::Symbol>,
        columns: usize,
        padding: f32,
        emoji: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let rows = values.len().div_ceil(columns);
        let view = cx.entity().downgrade();
        let grid = uniform_list("emoji-grid", rows, move |range, window, cx| {
            view.update(cx, |_this, cx| {
                range
                    .map(|row| {
                        div().h(px(47.)).flex().gap(px(4.)).children(
                            values.iter().skip(row * columns).take(columns).map(|item| {
                                let value = item.symbol.clone();
                                let (content, supported) = symbol_display(item, emoji, window, cx);
                                Button::new(SharedString::from(format!("emoji-{}", item.symbol)))
                                    .ghost()
                                    .rounded(px(7.))
                                    .child(content)
                                    .tooltip(if supported {
                                        item.name.clone()
                                    } else {
                                        message("{}（当前系统不支持显示，仍可插入）", &[("", item.name.clone())])
                                    })
                                    .accessibility_label(item.name.clone())
                                    .w(px(43.))
                                    .h(px(43.))
                                    .on_click(cx.listener(move |t, _, w, cx| {
                                        t.insert_symbol(value.clone(), w, cx)
                                    }))
                            }),
                        )
                    })
                    .collect()
            })
            .unwrap_or_default()
        })
        .flex_1()
        .min_h_0()
        .px(px(padding));
        grid.into_any_element()
    }

    fn insert_symbol(&self, symbol: String, window: &mut Window, cx: &mut App) {
        let target = (self.kind == Kind::Quick).then_some(self.target);
        if target.is_some() {
            hide(window, cx);
        }
        self.service.send(Command::Text(symbol.clone(), target));
        self.update_settings(|s| {
            s.recent_symbols.retain(|v| v != &symbol);
            s.recent_symbols.insert(0, symbol);
            s.recent_symbols.truncate(24);
        });
    }
}
