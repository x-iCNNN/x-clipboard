use super::*;
use gpui_kit::component::{
    menu::{DropdownMenu, PopupMenuItem},
    switch::Switch,
};

impl Desktop {
    #[inline(never)]
    pub(in crate::ui) fn settings(&mut self, cx: &mut Context<Self>) -> AnyElement {
        let (title, description) = match self.route {
            "appearance" => ("外观", "设置主题与显示密度，让内容更容易浏览。"),
            "shortcuts" => ("快捷键", "配置全局唤起快捷键，查看面板内操作。"),
            "storage" => ("存储", "管理内容容量、保留规则与数据目录。"),
            "privacy" => ("隐私", "管理加密保护与不需要记录的应用。"),
            "sync" => ("同步", "在你的设备之间，延续工作。"),
            "about" => ("关于", "为专注而设计，为日常而存在。"),
            _ => ("通用", "配置启动、窗口行为与记录状态。"),
        };
        let mut body = div()
            .id("settings-content")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px(px(32.))
            .pb(px(28.))
            .flex()
            .flex_col()
            .gap(px(22.));
        body = body.child(match self.route {
            "appearance" => self.settings_appearance(cx),
            "shortcuts" => self.settings_shortcuts(cx),
            "storage" => self.settings_storage(cx),
            "privacy" => self.settings_privacy(cx),
            "sync" => self.sync_page(cx),
            "about" => self.about_page(cx),
            _ => self.settings_general(cx),
        });
        if matches!(self.route, "shortcuts" | "storage" | "privacy") {
            body = body.child(
                div().flex().justify_end().child(
                    Button::new("save-settings")
                        .primary()
                        .label(tr("保存设置"))
                        .on_click(cx.listener(|t, _, _, cx| t.save_inputs(cx))),
                ),
            );
        }
        div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(page_heading(title, description, cx))
            .child(body)
            .into_any_element()
    }

    #[inline(never)]
    fn settings_general(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.snapshot.settings;
        let mut body = div().flex().flex_col().flex_shrink_0().gap(px(22.));
        let mut rows = vec![
            self.switch_row(
                "登录时启动",
                "在系统登录后自动开始记录。",
                s.start_at_login,
                |s, on| s.start_at_login = on,
                false,
                cx,
            ),
            setting_row(
                "关闭窗口后保留在托盘",
                "后台继续记录，可通过快捷键随时唤起。",
                badge("始终开启", true, cx),
                !cfg!(windows),
                cx,
            ),
        ];
        #[cfg(windows)]
        rows.push(setting_row(
            "快捷面板位置",
            "每次唤起时显示的位置。",
            self.setting_choice(
                "position",
                &s.position,
                &[("cursor", "鼠标所在位置"), ("center", "当前屏幕中央")],
                |s, v| s.position = v.into(),
            ),
            true,
            cx,
        ));
        body = body
            .child(section("语言", vec![setting_row(
                "界面语言", "语言切换后立即生效并自动保存。",
                self.language_picker("settings-language"), true, cx,
            )], cx))
            .child(section("启动与窗口", rows, cx))
            .child(section(
                "记录与反馈",
                vec![
                    self.switch_row(
                        "暂停记录",
                        "暂停期间不会保存新复制的内容。",
                        s.paused,
                        |s, on| s.paused = on,
                        false,
                        cx,
                    ),
                    setting_row(
                        "保存失败提示",
                        "记录无法保存时给出可操作的提示。",
                        badge("始终显示", false, cx),
                        true,
                        cx,
                    ),
                ],
                cx,
            ))
            .child(notice(
                IconName::Keyboard,
                &message("使用 {} 唤起快捷面板。点击记录上的预览按钮，查看完整内容。", &[("", display_shortcut(&s.shortcut))]),
                false,
                cx,
            ));
        body.into_any_element()
    }

    #[inline(never)]
    fn settings_appearance(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.snapshot.settings;
        let mut body = div().flex().flex_col().flex_shrink_0().gap(px(22.));
        body = body
            .child(
                div()
                    .flex()
                    .flex_col()
                    .gap(px(9.))
                    .child(section_heading("火绒黄 · 默认外观"))
                    .child(
                        div().flex().gap(px(12.)).children(
                            [("light", "浅色"), ("dark", "深色"), ("system", "跟随系统")]
                                .into_iter()
                                .map(|(theme, label)| {
                                    Button::new(theme)
                                        .ghost()
                                        .flex_1()
                                        .min_w_0()
                                        .h(px(132.))
                                        .p(px(12.))
                                        .border_1()
                                        .rounded(px(9.))
                                        .border_color(if s.theme == theme {
                                            accent(cx)
                                        } else {
                                            cx.theme().border
                                        })
                                        .bg(cx.theme().background)
                                        .child(
                                            div()
                                                .w_full()
                                                .flex()
                                                .flex_col()
                                                .gap(px(12.))
                                                .child(theme_thumbnail(theme))
                                                .child(
                                                    div()
                                                        .flex()
                                                        .items_center()
                                                        .justify_between()
                                                        .text_size(px(12.))
                                                        .font_weight(FontWeight::MEDIUM)
                                                        .child(tr(label).to_owned())
                                                        .when(s.theme == theme, |d| {
                                                            d.child(
                                                                Icon::new(IconName::Check)
                                                                    .size(px(14.))
                                                                    .text_color(accent(cx)),
                                                            )
                                                        }),
                                                ),
                                        )
                                        .on_click(cx.listener(move |t, _, _, _| {
                                            t.update_settings(|s| s.theme = theme.into())
                                        }))
                                }),
                        ),
                    ),
            )
            .child(section(
                "内置主题",
                vec![setting_row(
                    "GPUI Kit 主题",
                    "21 套主题，36 款明暗配色；选择后立即生效并保存。",
                    Button::new("builtin-theme")
                        .small()
                        .label(if ["system", "light", "dark"].contains(&s.theme.as_str()) {
                            tr("火绒黄（默认）").to_owned()
                        } else {
                            s.theme.clone()
                        })
                        .icon(IconName::ChevronDown)
                        .dropdown_menu({
                            let service = self.service.clone();
                            let selected = s.theme.clone();
                            move |menu, _, cx| theme_menu(menu, service.clone(), &selected, cx)
                        })
                        .into_any_element(),
                    true,
                    cx,
                )],
                cx,
            ))
            .child(section(
                "显示",
                vec![
                    setting_row(
                        "列表密度",
                        "调整历史记录之间的留白。",
                        self.setting_choice(
                            "density",
                            if s.compact { "compact" } else { "standard" },
                            &[("standard", "舒适"), ("compact", "紧凑")],
                            |s, v| s.compact = v == "compact",
                        ),
                        false,
                        cx,
                    ),
                    setting_row(
                        "界面语言",
                        "语言切换后立即生效并自动保存。",
                        self.language_picker("appearance-language"),
                        true,
                        cx,
                    ),
                ],
                cx,
            ))
            .child(notice(
                IconName::Monitor,
                "窗口按钮与标题栏遵循当前系统习惯。明暗主题也可通过窗口右上角快速切换。",
                false,
                cx,
            ));
        body.into_any_element()
    }

    #[inline(never)]
    fn settings_shortcuts(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.snapshot.settings;
        let mut body = div().flex().flex_col().flex_shrink_0().gap(px(22.));
        let global_note = if cfg!(windows) {
            "应用运行时接管设置的组合键；修改快捷键或退出后，系统原有快捷键恢复。"
        } else if cfg!(target_os = "macos") {
            "自动粘贴需要辅助功能权限。快捷键与权限可分别设置。"
        } else {
            "全局快捷键由桌面环境提供支持；受限环境可从托盘打开。"
        };
        let shortcut_status = if self
            .snapshot
            .error
            .as_ref()
            .is_some_and(|e| e.contains("快捷键") || e.contains("冲突"))
        {
            "需要检查"
        } else {
            "已应用"
        };
        body = body
            .child(notice(IconName::Keyboard, global_note, false, cx))
            .child(section(
                "全局快捷键",
                vec![
                    setting_row(
                        "打开快捷面板",
                        "点击右侧组合键，录入新的快捷键。",
                        Button::new("record-key")
                            .child(if self.recording_shortcut {
                                div()
                                    .text_size(px(12.))
                                    .child(tr("请按下快捷键…"))
                                    .into_any_element()
                            } else {
                                keys(&self.shortcut.read(cx).value(), false, cx)
                            })
                            .h(px(36.))
                            .min_w(px(132.))
                            .on_click(cx.listener(|t, _, w, cx| {
                                t.recording_shortcut = true;
                                t.focus.focus(w, cx);
                                cx.notify();
                            }))
                            .into_any_element(),
                        false,
                        cx,
                    ),
                    setting_row(
                        "快捷键状态",
                        "当前已保存并应用的组合键。",
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(keys(&s.shortcut, false, cx))
                            .child(badge(shortcut_status, shortcut_status == "已应用", cx))
                            .into_any_element(),
                        true,
                        cx,
                    ),
                ],
                cx,
            ))
            .child(section(
                "面板内快捷键",
                vec![
                    setting_row("粘贴选中内容", "", keys("Enter", false, cx), false, cx),
                    setting_row(
                        "以纯文本粘贴",
                        "",
                        keys("Shift+Enter", false, cx),
                        false,
                        cx,
                    ),
                    setting_row("展开 / 关闭预览", "", keys("Space", false, cx), false, cx),
                    setting_row("关闭预览或面板", "", keys("Esc", false, cx), true, cx),
                ],
                cx,
            ))
            .child(
                div()
                    .flex()
                    .items_center()
                    .justify_between()
                    .gap_3()
                    .child(value("输入框获得焦点时，文字输入优先。", cx))
                    .child(
                        Button::new("default-key")
                            .ghost()
                            .small()
                            .icon(IconName::RefreshCw)
                            .label(tr("恢复默认"))
                            .on_click(cx.listener(|t, _, w, cx| {
                                t.shortcut.update(cx, |i, cx| {
                                    i.set_value(Settings::default().shortcut, w, cx)
                                });
                                t.save_inputs(cx);
                            })),
                    ),
            );
        body.into_any_element()
    }

    #[inline(never)]
    fn settings_storage(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.snapshot.settings;
        let mut body = div().flex().flex_col().flex_shrink_0().gap(px(22.));
        let used = self.snapshot.entries.iter().map(|e| e.bytes).sum::<u64>();
        let images = self
            .snapshot
            .entries
            .iter()
            .filter(|e| e.kind == "image")
            .map(|e| e.bytes)
            .sum::<u64>();
        let text = self
            .snapshot
            .entries
            .iter()
            .filter(|e| e.kind == "text" || e.kind == "rich")
            .map(|e| e.bytes)
            .sum::<u64>();
        let protected = self
            .snapshot
            .entries
            .iter()
            .filter(|e| e.pinned || e.favorite)
            .map(|e| e.bytes)
            .sum::<u64>();
        let denominator = s.max_bytes.max(used).max(1) as f32;
        let usage = div()
            .p(px(20.))
            .flex()
            .flex_col()
            .gap(px(16.))
            .child(div().flex().gap(px(26.)).children([
                usage_stat(
                    &bytes_label(used),
                    &message("内容容量 / {}", &[("", bytes_label(s.max_bytes))]),
                    cx,
                ),
                usage_stat(
                    &self.snapshot.entries.len().to_string(),
                    &message("历史记录 / {} 条", &[("", s.max_entries.to_string())]),
                    cx,
                ),
                usage_stat(&bytes_label(self.snapshot.disk_bytes), "实际磁盘占用", cx),
            ]))
            .child(
                div()
                    .w_full()
                    .h(px(7.))
                    .rounded_full()
                    .overflow_hidden()
                    .bg(cx.theme().muted)
                    .flex()
                    .child(
                        div()
                            .h_full()
                            .w(relative(images as f32 / denominator))
                            .bg(rgb(0xdca400)),
                    )
                    .child(
                        div()
                            .h_full()
                            .w(relative(text as f32 / denominator))
                            .bg(rgb(0x89bda8)),
                    )
                    .child(
                        div()
                            .h_full()
                            .w(relative(
                                used.saturating_sub(images + text) as f32 / denominator,
                            ))
                            .bg(rgb(0xc2d8cc)),
                    ),
            )
            .child(
                div().flex().flex_wrap().gap(px(18.)).children(
                    [
                        (0xdca400, "图片"),
                        (0x89bda8, "文本与富文本"),
                        (0xc2d8cc, "文件引用与其他"),
                    ]
                    .into_iter()
                    .map(|(color, label)| {
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().size(px(6.)).rounded(px(2.)).bg(rgb(color)))
                            .child(value(label, cx))
                    }),
                ),
            )
            .into_any_element();
        body = body
            .child(section("内容占用", vec![usage], cx))
            .when(
                used > s.max_bytes || self.snapshot.entries.len() > s.max_entries,
                |d| {
                    d.child(notice(
                        IconName::Shield,
                        &message("置顶与收藏占用 {}，不会自动删除。请提高上限，或手动整理受保护记录。", &[("", bytes_label(protected))]),
                        true,
                        cx,
                    ))
                },
            )
            .child(section(
                "保留规则",
                vec![
                    setting_row(
                        "最多保留记录数",
                        "超过上限时清理最旧的普通记录。",
                        div()
                            .w(px(118.))
                            .child(Input::new(&self.limit).small())
                            .into_any_element(),
                        false,
                        cx,
                    ),
                    setting_row(
                        "内容容量上限",
                        "置顶与收藏不参与自动清理。",
                        div()
                            .flex()
                            .items_center()
                            .gap_2()
                            .child(div().w(px(118.)).child(Input::new(&self.capacity).small()))
                            .child(value("MB", cx))
                            .into_any_element(),
                        true,
                        cx,
                    ),
                ],
                cx,
            ))
            .child(section(
                "本地数据",
                vec![
                    setting_row(
                        "数据目录",
                        &self.service.root.display().to_string(),
                        Button::new("open-data-folder")
                            .small()
                            .icon(IconName::Folder)
                            .label(tr("打开目录"))
                            .on_click(cx.listener(|t, _, _, _| {
                                let path = t.service.root.clone();
                                let service = t.service.clone();
                                std::thread::spawn(move || {
                                    if let Err(e) = open::that(path.as_ref()) {
                                        service.error(format!("无法打开数据目录：{e}"));
                                    }
                                });
                            }))
                            .into_any_element(),
                        false,
                        cx,
                    ),
                    setting_row(
                        "清空历史",
                        "默认保留置顶与收藏。",
                        Button::new("clear-settings")
                            .small()
                            .icon(IconName::Trash)
                            .label(tr("清空…"))
                            .text_color(rgb(0xc84949))
                            .on_click(cx.listener(|t, _, w, cx| {
                                t.clear_protected = false;
                                t.confirm = Some(true);
                                t.dialog_focus.focus(w, cx);
                                cx.notify();
                            }))
                            .into_any_element(),
                        true,
                        cx,
                    ),
                ],
                cx,
            ))
            .child(value(
                "文件只保存引用，原文件大小不计入内容容量。磁盘占用包含数据库维护空间。",
                cx,
            ));
        body.into_any_element()
    }

    #[inline(never)]
    fn settings_privacy(&self, cx: &mut Context<Self>) -> AnyElement {
        let s = &self.snapshot.settings;
        let mut body = div().flex().flex_col().flex_shrink_0().gap(px(22.));
        let key_store = if cfg!(windows) {
            "Windows DPAPI"
        } else if cfg!(target_os = "macos") {
            "Keychain"
        } else {
            "Secret Service"
        };
        body = body
            .child(section(
                "加密保护",
                vec![
                    setting_row(
                        "本地加密",
                        "记录、图片与持久化预览均加密保存。",
                        badge(
                            if self.snapshot.ready {
                                "已启用"
                            } else {
                                "尚未解锁"
                            },
                            self.snapshot.ready,
                            cx,
                        ),
                        false,
                        cx,
                    ),
                    setting_row(
                        "密钥保护",
                        "由系统账户密钥库保护，不以明文保存。",
                        value(key_store, cx),
                        true,
                        cx,
                    ),
                ],
                cx,
            ))
            .child(section(
                "记录控制",
                vec![
                    self.switch_row(
                        "尊重敏感内容标记",
                        "应用明确标记为敏感的内容，不写入历史。",
                        s.respect_sensitive,
                        |s, on| s.respect_sensitive = on,
                        false,
                        cx,
                    ),
                    div()
                        .px(px(16.))
                        .py(px(14.))
                        .flex()
                        .flex_col()
                        .gap_3()
                        .child(
                            div()
                                .text_size(px(14.))
                                .font_weight(FontWeight::MEDIUM)
                                .child(tr("排除的应用")),
                        )
                        .child(value(
                            if cfg!(windows) {
                                "填写可执行文件名，以英文逗号分隔；已有历史不会被删除。"
                            } else {
                                "当前平台尚不支持来源应用排除。"
                            },
                            cx,
                        ))
                        .child(Input::new(&self.excluded).small().disabled(!cfg!(windows)))
                        .into_any_element(),
                ],
                cx,
            ))
            .child(notice(
                IconName::Shield,
                "所有内容默认留在本机。日志不包含剪贴板正文或加密密钥。",
                false,
                cx,
            ));
        body.into_any_element()
    }

    #[inline(never)]
    fn switch_row(
        &self,
        title: &str,
        description: &str,
        on: bool,
        update: fn(&mut Settings, bool),
        last: bool,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        setting_row(
            title,
            description,
            div()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .text_size(px(11.))
                        .text_color(if on {
                            accent(cx)
                        } else {
                            cx.theme().muted_foreground
                        })
                        .child(tr(if on { "已开启" } else { "已关闭" })),
                )
                .child(
                    Switch::new(SharedString::from(format!("setting-{title}")))
                        .small()
                        .checked(on)
                        .color(cx.theme().button_primary)
                        .accessibility_label(tr(title).to_owned())
                        .on_click(cx.listener(move |t, on: &bool, _, _| {
                            t.update_settings(|s| update(s, *on))
                        })),
                )
                .into_any_element(),
            last,
            cx,
        )
    }

    #[inline(never)]
    fn setting_choice(
        &self,
        id: &'static str,
        selected: &str,
        options: &'static [(&'static str, &'static str)],
        update: fn(&mut Settings, &str),
    ) -> AnyElement {
        let selected = selected.to_owned();
        let label = options
            .iter()
            .find(|(value, _)| *value == selected)
            .map(|(_, label)| *label)
            .unwrap_or("请选择");
        let service = self.service.clone();
        Button::new(id)
            .small()
            .child(
                div()
                    .flex()
                    .items_center()
                    .gap_3()
                    .child(tr(label).to_owned())
                    .child(Icon::new(IconName::ChevronDown).size(px(14.))),
            )
            .dropdown_menu(move |mut menu, _, _| {
                for (value, label) in options {
                    let service = service.clone();
                    menu = menu.item(
                        PopupMenuItem::new(tr(label))
                            .checked(*value == selected)
                            .on_click(move |_, _, _| {
                                let mut settings = service.snapshot().settings;
                                update(&mut settings, value);
                                service.send(Command::Settings(settings));
                            }),
                    );
                }
                menu
            })
            .into_any_element()
    }

    #[inline(never)]
    fn sync_page(&self, cx: &App) -> AnyElement {
        div().w_full().pt(px(22.)).flex().flex_col().items_center().gap(px(16.))
            .child(brand_icon(IconName::Cloud, 70., true, cx)).child(badge("计划中", false, cx))
            .child(div().text_size(px(24.)).font_weight(FontWeight::SEMIBOLD).child(tr("让内容跟随你的工作。")))
            .child(div().text_size(px(13.)).text_color(cx.theme().muted_foreground).text_center().child(tr("未来通过 WebDAV，在你的设备之间同步剪贴板。\n使用你选择的存储服务，保持对数据的掌控。")))
            .child(div().mt_3().w(px(380.)).p_5().border_1().border_color(cx.theme().border).rounded(px(10.)).bg(cx.theme().muted.opacity(0.35)).flex().flex_col().gap(px(16.)).children([
                (IconName::Monitor, "跨设备延续工作"), (IconName::Shield, "同步前加密，隐私由你掌握"), (IconName::Cloud, "连接自己的 WebDAV 服务"),
            ].into_iter().map(|(icon, label)| div().flex().items_center().gap_3().text_size(px(13.)).child(Icon::new(icon).size(px(17.)).text_color(accent(cx))).child(tr(label).to_owned()))))
            .child(div().mt_3().child(value("当前版本仅保存在本机，尚未连接任何同步服务。", cx))).into_any_element()
    }

    #[inline(never)]
    fn about_page(&self, cx: &App) -> AnyElement {
        let platform = if cfg!(windows) {
            "Windows 10 / 11"
        } else if cfg!(target_os = "macos") {
            "macOS 15+"
        } else {
            "Linux X11 / Wayland"
        };
        let license = "GPUI Kit 0.8.0 · Apache-2.0\nTurso 0.8.3 · MIT\nPDFium · BSD-style\nimage / tray-icon · MIT / Apache-2.0\nUnicode / CLDR · Unicode License v3\n\n完整依赖版本见 Cargo.lock，资源许可随分发包保留。";
        let help = "复制后按全局快捷键唤起面板。\n单击选中；双击或 Enter 粘贴。\nShift+Enter 以纯文本粘贴。\n眼睛按钮或 Space 展开预览。\n图钉置顶；星标收藏，保留常用内容。\n\n文件只保存引用，不备份原文件。\n快捷面板回贴原窗口，管理窗口只复制。\n受限环境中请复制后手动粘贴。";
        div()
            .flex()
            .flex_col()
            .gap(px(22.))
            .child(
                div()
                    .pt_1()
                    .pb_3()
                    .flex()
                    .flex_col()
                    .items_center()
                    .gap(px(9.))
                    .child(app_mark(58.))
                    .child(
                        div()
                            .mt_2()
                            .text_size(px(24.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child("X Clipboard"),
                    )
                    .child(value("每一次复制，都有迹可循。", cx))
                    .child(badge("0.1.0 · 原生测试版", false, cx)),
            )
            .child(section(
                "应用信息",
                vec![
                    setting_row("运行平台", "", value(platform, cx), false, cx),
                    setting_row(
                        "界面与数据",
                        "本机处理，离线优先。",
                        value("Rust · GPUI Kit · Turso", cx),
                        true,
                        cx,
                    ),
                ],
                cx,
            ))
            .child(section(
                "开源与帮助",
                vec![
                    setting_row(
                        "开源组件许可",
                        "查看框架、预览引擎与其他依赖。",
                        info_popover("licenses", "开源组件许可", license, cx),
                        false,
                        cx,
                    ),
                    setting_row(
                        "使用帮助",
                        "快捷键、记录规则与平台能力。",
                        info_popover("help", "使用帮助", help, cx),
                        true,
                        cx,
                    ),
                    setting_row(
                        "使用指南",
                        "重新查看首次使用说明。",
                        Button::new("open-guide")
                            .small()
                            .label(tr("查看使用指南"))
                            .on_click(|_, _, cx| open_window(Kind::Welcome, cx))
                            .into_any_element(),
                        true,
                        cx,
                    ),
                ],
                cx,
            ))
            .child(
                div()
                    .text_center()
                    .text_size(px(10.))
                    .text_color(cx.theme().muted_foreground)
                    .child("DESIGNED FOR YOUR EVERYDAY FLOW."),
            )
            .into_any_element()
    }
}

fn theme_thumbnail(theme: &str) -> AnyElement {
    let dark = theme == "dark";
    let line = if dark { 0x42494d } else { 0xd9dfe2 };
    div()
        .w_full()
        .h(px(72.))
        .border_1()
        .border_color(rgb(line))
        .rounded(px(5.))
        .overflow_hidden()
        .flex()
        .flex_col()
        .child(
            div()
                .h(px(10.))
                .flex_shrink_0()
                .bg(rgb(if dark { 0x1d2125 } else { 0xf1f3f4 }))
                .border_b_1()
                .border_color(rgb(line)),
        )
        .child(
            div()
                .flex_1()
                .flex()
                .bg(rgb(if dark { 0x25292d } else { 0xfafbfc }))
                .child(
                    div()
                        .w(px(38.))
                        .h_full()
                        .border_r_1()
                        .border_color(rgb(line))
                        .bg(rgb(if dark { 0x1d2125 } else { 0xedf0f1 })),
                )
                .child(
                    div()
                        .flex_1()
                        .p(px(10.))
                        .flex()
                        .flex_col()
                        .gap(px(7.))
                        .bg(rgb(if dark || theme == "system" {
                            0x25292d
                        } else {
                            0xfafbfc
                        }))
                        .child(div().h(px(8.)).w_full().rounded(px(2.)).bg(rgb(
                            if dark || theme == "system" {
                                0x61502a
                            } else {
                                0xffedb4
                            },
                        )))
                        .child(div().h(px(4.)).w(px(34.)).rounded(px(2.)).bg(rgb(
                            if dark || theme == "system" {
                                0x42494d
                            } else {
                                0xe1e6e9
                            },
                        ))),
                ),
        )
        .into_any_element()
}
fn usage_stat(number: &str, caption: &str, cx: &App) -> AnyElement {
    div()
        .flex_1()
        .min_w_0()
        .flex()
        .flex_col()
        .gap(px(5.))
        .child(
            div()
                .text_size(px(24.))
                .font_weight(FontWeight::SEMIBOLD)
                .child(number.to_owned()),
        )
        .child(value(caption, cx))
        .into_any_element()
}
fn info_popover(
    id: &'static str,
    title: &'static str,
    content: &'static str,
    cx: &App,
) -> AnyElement {
    Popover::new(id)
        .anchor(Anchor::BottomRight)
        .trigger(
            Button::new(SharedString::from(format!("open-{id}")))
                .ghost()
                .small()
                .icon(IconName::ChevronRight)
                .accessibility_label(tr(title).to_owned()),
        )
        .child(
            div()
                .w(px(360.))
                .p(px(18.))
                .flex()
                .flex_col()
                .gap(px(14.))
                .child(
                    div()
                        .text_size(px(16.))
                        .font_weight(FontWeight::SEMIBOLD)
                        .child(tr(title).to_owned()),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .child(tr(content)),
                ),
        )
        .into_any_element()
}
