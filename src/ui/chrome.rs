use super::*;

impl Desktop {
    pub(super) fn sidebar(&self, cx: &mut Context<Self>) -> AnyElement {
        let settings = self.kind == Kind::Settings;
        let mut column = div()
            .w(px(if settings { 180. } else { 200. }))
            .h_full()
            .flex_shrink_0()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .flex()
            .flex_col()
            .px_3();
        column = column.child(
            div()
                .h(px(62.))
                .flex_shrink_0()
                .px_2()
                .flex()
                .items_center()
                .gap_3()
                .child(if settings {
                    Icon::new(IconName::Settings)
                        .size(px(22.))
                        .text_color(accent(cx))
                        .into_any_element()
                } else {
                    app_mark(26.)
                })
                .child(
                    div()
                        .text_size(px(13.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr(if settings { "设置" } else { "X Clipboard" })),
                ),
        );
        let routes = if settings {
            vec![
                ("general", "通用设置", IconName::Settings),
                ("appearance", "外观设置", IconName::Sun),
                ("shortcuts", "快捷键", IconName::Keyboard),
                ("storage", "本地存储", IconName::Save),
                ("privacy", "隐私保护", IconName::Shield),
                ("sync", "同步", IconName::RefreshCw),
                ("about", "关于", IconName::Info),
            ]
        } else {
            vec![
                ("all", "全部历史", IconName::Clipboard),
                ("pinned", "置顶", IconName::Pin),
                ("favorites", "收藏", IconName::Star),
                ("emoji", "表情", IconName::FaceSlightlySmiling),
                ("symbols", "符号", IconName::Hash),
            ]
        };
        for (route, label, icon) in routes {
            if !settings && route == "all" {
                column = column.child(nav_heading("剪贴板", cx));
            }
            if route == "emoji" {
                column = column
                    .child(div().mx_3().mt_4().mb_2().h(px(1.)).bg(cx.theme().border))
                    .child(nav_heading("快速输入", cx));
            }
            let count = match route {
                "all" => Some(self.snapshot.entries.len()),
                "pinned" => Some(self.snapshot.entries.iter().filter(|e| e.pinned).count()),
                "favorites" => Some(self.snapshot.entries.iter().filter(|e| e.favorite).count()),
                _ => None,
            };
            let active = self.route == route;
            column = column.child(
                Button::new(SharedString::from(format!("nav-{route}")))
                    .ghost()
                    .w_full()
                    .h(px(40.))
                    .my(px(2.))
                    .px_3()
                    .rounded(px(7.))
                    .accessibility_label(tr(label).to_owned())
                    .tooltip(tr(label).to_owned())
                    .text_color(if active {
                        accent(cx)
                    } else {
                        cx.theme().muted_foreground
                    })
                    .when(active, |b| b.bg(accent_soft(cx)))
                    .child(
                        div()
                            .w_full()
                            .flex()
                            .items_center()
                            .gap_3()
                            .when(route != "symbols", |d| {
                                d.child(Icon::new(icon).size(px(18.)).flex_shrink_0())
                            })
                            .when(route == "symbols", |d| {
                                d.child(div().w(px(18.)).text_size(px(20.)).child("Σ"))
                            })
                            .child(
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .truncate()
                                    .text_size(px(13.))
                                    .when(active, |d| d.font_weight(FontWeight::SEMIBOLD))
                                    .child(tr(label).to_owned()),
                            )
                            .when_some(count, |d, count| {
                                d.child(
                                    div()
                                        .flex_shrink_0()
                                        .text_size(px(10.))
                                        .child(count.to_string()),
                                )
                            }),
                    )
                    .on_click(cx.listener(move |t, _, _, cx| t.nav(route, cx))),
            );
        }
        column = column.child(div().flex_1());
        if settings {
            column = column
                .child(div().h(px(1.)).mx_2().bg(cx.theme().border))
                .child(
                    Button::new("settings-back")
                        .ghost()
                        .h(px(48.))
                        .w_full()
                        .justify_start()
                        .icon(IconName::ChevronLeft)
                        .label(tr("返回剪贴板"))
                        .on_click(|_, w, cx| {
                            hide(w, cx);
                            open_window(Kind::Main, cx);
                        }),
                );
        }
        column.into_any_element()
    }
    pub(super) fn footer(&self, window: &Window, cx: &mut Context<Self>) -> AnyElement {
        if self.kind == Kind::Quick {
            return div()
                .relative()
                .h(px(38.))
                .flex_shrink_0()
                .px_3()
                .bg(cx.theme().sidebar)
                .border_t_1()
                .border_color(cx.theme().border)
                .flex()
                .items_center()
                .gap_1()
                .text_size(px(10.))
                .text_color(cx.theme().muted_foreground)
                .child(keycap("↵", cx))
                .child(tr("粘贴"))
                .child(div().w(px(3.)))
                .child(keycap("Shift ↵", cx))
                .child(tr("纯文本"))
                .child(div().w(px(3.)))
                .child(keycap("Esc", cx))
                .child(tr("关闭"))
                .child(div().flex_1())
                .child(
                    icon_button("quick-settings", IconName::Settings, "设置")
                        .on_click(|_, _, cx| open_window(Kind::Settings, cx)),
                )
                .into_any_element();
        }
        let status = if self.snapshot.error.is_some() {
            "需要处理"
        } else if self.snapshot.settings.paused {
            "已暂停记录"
        } else if self.snapshot.status.contains("保存中") {
            "保存中…"
        } else if self.snapshot.status.starts_with("已复制")
            || self.snapshot.status.starts_with("已向原窗口")
        {
            self.snapshot.status.as_str()
        } else {
            "正在记录"
        };
        div()
            .h(px(52.))
            .flex_shrink_0()
            .bg(cx.theme().sidebar)
            .border_t_1()
            .border_color(cx.theme().border)
            .flex()
            .items_center()
            .text_size(px(11.))
            .text_color(cx.theme().muted_foreground)
            .child(
                div()
                    .w(px(200.))
                    .h_full()
                    .border_r_1()
                    .border_color(cx.theme().border)
                    .flex_shrink_0()
                    .px_3()
                    .flex()
                    .items_center()
                    .child(
                        Button::new("open-settings")
                            .ghost()
                            .w_full()
                            .justify_start()
                            .icon(Icon::new(IconName::Settings).size(px(18.)))
                            .label(tr("设置"))
                            .on_click(|_, _, cx| open_window(Kind::Settings, cx)),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .px_4()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .size(px(6.))
                            .rounded_full()
                            .bg(if self.snapshot.error.is_some() {
                                danger(cx)
                            } else if self.snapshot.settings.paused {
                                warning(cx)
                            } else {
                                accent(cx)
                            }),
                    )
                    .child(tr(status).to_owned())
                    .child(
                        Button::new("pause-recording")
                            .ghost()
                            .small()
                            .h(px(30.))
                            .label(tr(if self.snapshot.settings.paused {
                                "恢复"
                            } else {
                                "暂停"
                            }))
                            .on_click(cx.listener(|t, _, _, _| {
                                t.update_settings(|s| s.paused = !s.paused)
                            })),
                    )
                    .child(div().flex_1())
                    .child(Icon::new(IconName::Clipboard).size(px(13.)))
                    .child(message(
                        "已保存 {} 条记录",
                        &[("", self.snapshot.entries.len().to_string())],
                    ))
                    .child(div().flex_1())
                    .child(Icon::new(IconName::Lock).size(px(13.)))
                    .child(if f32::from(window.viewport_size().width) < 1000. {
                        div().child(tr("已加密")).into_any_element()
                    } else {
                        div()
                            .flex()
                            .items_center()
                            .gap_1()
                            .child(tr("本地加密 ·"))
                            .child(keycap(self.snapshot.settings.shortcut.clone(), cx))
                            .child(tr("唤起"))
                            .into_any_element()
                    }),
            )
            .into_any_element()
    }
}
fn nav_heading(label: &'static str, cx: &App) -> AnyElement {
    div()
        .h(px(30.))
        .px_3()
        .flex()
        .items_center()
        .text_size(px(11.))
        .text_color(cx.theme().muted_foreground)
        .child(tr(label).to_owned())
        .into_any_element()
}
