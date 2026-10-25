use super::*;

impl Desktop {
    #[inline(never)]
    pub(in crate::ui) fn welcome(
        &mut self,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let sidebar = self.welcome_sidebar(cx);
        let content = self.welcome_content(cx);
        div()
            .size_full()
            .flex()
            .child(sidebar)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .flex()
                    .flex_col()
                    .child(self.titlebar(window, cx))
                    .child(content),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn welcome_sidebar(&self, cx: &App) -> AnyElement {
        let left = div()
            .w(px(220.))
            .h_full()
            .flex_shrink_0()
            .bg(cx.theme().sidebar)
            .border_r_1()
            .border_color(cx.theme().border)
            .px(px(25.))
            .pt(px(35.))
            .pb(px(30.))
            .flex()
            .flex_col()
            .child(div().w_full().flex().justify_center().child(app_mark(58.)))
            .child(
                div()
                    .mt(px(16.))
                    .text_size(px(18.))
                    .font_weight(FontWeight::BOLD)
                    .w_full()
                    .text_center()
                    .child("X Clipboard"),
            )
            .child(
                div()
                    .mt(px(7.))
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(tr("留住内容，\n继续专注。")),
            )
            .child(
                div()
                    .mt(px(14.))
                    .w_full()
                    .flex()
                    .justify_center()
                    .child(self.language_picker("welcome-language")),
            )
            .child(
                div().mt(px(32.)).flex().flex_col().gap(px(18.)).children(
                    ["欢迎使用", "设置快捷键", "系统能力", "本地保护"]
                        .into_iter()
                        .enumerate()
                        .map(|(i, label)| {
                            let active = self.step == i;
                            div()
                                .flex()
                                .items_center()
                                .gap(px(9.))
                                .text_size(px(12.))
                                .text_color(if active {
                                    accent(cx)
                                } else {
                                    cx.theme().muted_foreground
                                })
                                .when(active, |d| d.font_weight(FontWeight::SEMIBOLD))
                                .child(
                                    div()
                                        .size(px(22.))
                                        .flex_shrink_0()
                                        .rounded_full()
                                        .border_1()
                                        .border_color(if active {
                                            accent(cx)
                                        } else {
                                            cx.theme().border
                                        })
                                        .bg(if active {
                                            accent(cx)
                                        } else {
                                            cx.theme().background
                                        })
                                        .text_color(if active {
                                            cx.theme().background
                                        } else {
                                            cx.theme().muted_foreground
                                        })
                                        .flex()
                                        .items_center()
                                        .justify_center()
                                        .text_size(px(11.))
                                        .child(if self.step > i {
                                            "✓".into()
                                        } else {
                                            (i + 1).to_string()
                                        }),
                                )
                                .child(
                                    div()
                                        .flex_1()
                                        .min_w_0()
                                        .whitespace_normal()
                                        .child(tr(label)),
                                )
                        }),
                ),
            )
            .child(div().flex_1())
            .child(value("无需账户 · 默认离线", cx));

        left.into_any_element()
    }

    #[inline(never)]
    fn welcome_content(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div()
            .flex_1()
            .min_h_0()
            .px(px(32.))
            .pt(px(24.))
            .pb(px(25.))
            .flex()
            .flex_col();
        let (eyebrow, title, description) = match self.step {
            0 => (
                "WELCOME TO X CLIPBOARD",
                "复制之后，\n不必再担心找不到。",
                "文字、图片、文件，随手复制，随时找回。\n用一个安静可靠的工具，接住你的每个灵感。",
            ),
            1 => (
                "01 / YOUR SHORTCUT",
                "一个快捷键，\n找回刚才的内容。",
                "使用快捷键，在任意应用中唤起剪贴板。\n你也可以稍后在设置中修改。",
            ),
            2 => (
                "02 / PLATFORM ACCESS",
                "准备好，\n融入你的工作流。",
                "只使用记录和粘贴所需的系统能力。\n你可以随时在设置中调整。",
            ),
            _ => (
                "03 / PRIVATE BY DEFAULT",
                "从第一条记录开始，\n就受到保护。",
                "",
            ),
        };
        content = content
            .child(
                div()
                    .text_size(px(10.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(accent(cx))
                    .child(tr(eyebrow).to_owned()),
            )
            .child(
                div()
                    .mt(px(10.))
                    .text_size(px(26.))
                    .font_weight(FontWeight::BOLD)
                    .child(tr(title).to_owned()),
            )
            .when(!description.is_empty(), |d| {
                d.child(
                    div()
                        .mt(px(10.))
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .child(tr(description).to_owned()),
                )
            });
        content = content.child(match self.step {
            0 => self.welcome_intro(cx),
            1 => self.welcome_shortcut(cx),
            2 => self.welcome_capabilities(cx),
            _ => self.welcome_protection(cx),
        });
        content = content.child(div().flex_1()).child(
            div()
                .pt(px(18.))
                .flex()
                .items_center()
                .justify_between()
                .child(if self.step == 0 {
                    value("大约需要 1 分钟", cx)
                } else {
                    Button::new("welcome-back")
                        .ghost()
                        .small()
                        .icon(IconName::ChevronLeft)
                        .label(tr("上一步"))
                        .on_click(cx.listener(|t, _, _, cx| {
                            t.step = t.step.saturating_sub(1);
                            cx.notify();
                        }))
                        .into_any_element()
                })
                .child(
                    Button::new("welcome-next")
                        .primary()
                        .label(tr(if self.step == 3 {
                            "开始使用"
                        } else {
                            "继续"
                        }))
                        .icon(IconName::ArrowRight)
                        .disabled(!self.snapshot.ready)
                        .on_click(cx.listener(|t, _, w, cx| {
                            if t.step < 3 {
                                if t.step == 1 {
                                    t.save_inputs(cx);
                                }
                                t.step += 1;
                                cx.notify();
                            } else {
                                t.update_settings(|s| s.setup_complete = true);
                                hide(w, cx);
                                open_window(Kind::Main, cx);
                            }
                        })),
                ),
        );
        content.into_any_element()
    }

    #[inline(never)]
    fn welcome_intro(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div().flex().flex_col().flex_shrink_0();
        content = content.child(
            div().mt(px(20.)).flex().flex_col().gap(px(21.)).children(
                [
                    (
                        IconName::Clipboard,
                        "记住每一次复制",
                        "在后台保存历史，不打断当前工作。",
                    ),
                    (
                        IconName::Pin,
                        "让重要内容触手可及",
                        "置顶常用内容，收藏值得保留的片段。",
                    ),
                    (
                        IconName::Lock,
                        "内容只属于你",
                        "本地加密保存，不上传到云端。",
                    ),
                ]
                .into_iter()
                .map(|(icon, title, desc)| feature(icon, title, desc, cx)),
            ),
        );
        content.into_any_element()
    }

    #[inline(never)]
    fn welcome_shortcut(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div().flex().flex_col().flex_shrink_0();
        content = content
            .child(
                div()
                    .my(px(20.))
                    .p(px(20.))
                    .bg(cx.theme().muted.opacity(0.35))
                    .border_1()
                    .border_color(cx.theme().border)
                    .rounded(px(10.))
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(keys(&self.shortcut.read(cx).value(), true, cx)),
            )
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(badge(
                        if self.recording_shortcut {
                            "正在录入"
                        } else {
                            "可随时修改"
                        },
                        !self.recording_shortcut,
                        cx,
                    ))
                    .child(
                        Button::new("welcome-shortcut")
                            .ghost()
                            .small()
                            .icon(IconName::Keyboard)
                            .label(tr(if self.recording_shortcut {
                                "请按下快捷键…"
                            } else {
                                "修改快捷键"
                            }))
                            .on_click(cx.listener(|t, _, w, cx| {
                                t.recording_shortcut = true;
                                t.focus.focus(w, cx);
                                cx.notify();
                            })),
                    ),
            )
            .child(div().mt_3().child(value(
                if cfg!(windows) {
                    "应用运行时接管已设置组合；退出后恢复系统处理。"
                } else {
                    "系统权限与快捷键冲突会明确提示。"
                },
                cx,
            )));
        content.into_any_element()
    }

    #[inline(never)]
    fn welcome_capabilities(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div().flex().flex_col().flex_shrink_0();
        content = content
            .child(
                div().mt(px(14.)).children(
                    [
                        (
                            IconName::Clipboard,
                            "后台读取剪贴板",
                            if self.snapshot.ready {
                                "服务已启动"
                            } else {
                                "初始化中"
                            },
                        ),
                        (IconName::Keyboard, "全局快捷键", "按平台能力提供"),
                        (
                            IconName::ClipboardPaste,
                            "自动粘贴",
                            if cfg!(windows) {
                                "普通窗口可用"
                            } else {
                                "需要桌面权限"
                            },
                        ),
                        (
                            IconName::Lock,
                            "系统密钥库",
                            if self.snapshot.ready {
                                "已解锁"
                            } else {
                                "待解锁"
                            },
                        ),
                    ]
                    .into_iter()
                    .map(|(icon, label, state)| {
                        div()
                            .py(px(11.))
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .flex()
                            .items_center()
                            .gap_3()
                            .text_size(px(12.))
                            .child(
                                Icon::new(icon)
                                    .size(px(17.))
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(div().flex_1().child(tr(label).to_owned()))
                            .child(badge(
                                state,
                                self.snapshot.ready && matches!(state, "服务已启动" | "已解锁"),
                                cx,
                            ))
                    }),
                ),
            )
            .child(
                div()
                    .mt_3()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child(tr(self.service.backend.capabilities())),
            );
        content.into_any_element()
    }

    #[inline(never)]
    fn welcome_protection(&self, cx: &mut Context<Self>) -> AnyElement {
        let mut content = div().flex().flex_col().flex_shrink_0();
        content = content
            .child(
                div()
                    .py(px(16.))
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(8.))
                    .child(brand_icon(IconName::Shield, 54., true, cx))
                    .child(
                        div()
                            .text_size(px(15.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr(if self.snapshot.ready {
                                "本地加密已准备就绪"
                            } else {
                                "本地加密尚未就绪"
                            })),
                    )
                    .child(value("密钥由系统保护，内容留在你的设备上。", cx)),
            )
            .child(notice(
                IconName::Info,
                "默认保留 10,000 条或 2 GB 内容。\n置顶与收藏不会被自动清理。",
                false,
                cx,
            ))
            .child(
                div()
                    .mt_3()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(
                        Switch::new("welcome-startup")
                            .small()
                            .checked(self.snapshot.settings.start_at_login)
                            .color(cx.theme().button_primary)
                            .accessibility_label(tr("登录系统后自动启动"))
                            .on_click(cx.listener(|t, on: &bool, _, _| {
                                t.update_settings(|s| s.start_at_login = *on)
                            })),
                    )
                    .child(div().text_size(px(12.)).child(tr("登录系统后自动启动"))),
            );
        content.into_any_element()
    }
}
