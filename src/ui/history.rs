use super::*;

#[derive(Clone)]
enum HistoryLine {
    Heading(&'static str, usize),
    Entry(usize),
}

impl Desktop {
    pub(super) fn open_preview(&mut self, id: String, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_open = true;
        self.select(id, window, cx);
        if self.kind == Kind::Quick {
            let available = window
                .display(cx)
                .map(|d| f32::from(d.bounds().size.width))
                .unwrap_or(0.);
            if available >= QUICK_WIDTH + PREVIEW_WIDTH + 48. {
                resize_quick_window(window, QUICK_WIDTH + PREVIEW_WIDTH, QUICK_HEIGHT);
                #[cfg(windows)]
                platform::place_panel(native(window), "clamp");
            }
        }
        cx.notify();
    }
    pub(super) fn close_preview(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.preview_open = false;
        self.service.release_preview(self.preview_generation);
        self.snapshot.preview = None;
        self.preview_key.clear();
        if let Some(image) = self.image.take() {
            let _ = window.drop_image(image);
        }
        self.editor.update(cx, |e, cx| e.set_value("", window, cx));
        if self.kind == Kind::Quick {
            resize_quick_window(window, QUICK_WIDTH, QUICK_HEIGHT);
        }
        cx.notify();
    }
    #[inline(never)]
    pub(super) fn history(&mut self, window: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let quick = self.kind == Kind::Quick;
        if self.selected.as_ref().is_some_and(|id| {
            !self
                .visible(cx)
                .iter()
                .any(|index| &self.snapshot.entries[*index].id == id)
        }) {
            self.selected = None;
            self.preview_key.clear();
            if let Some(image) = self.image.take() {
                let _ = window.drop_image(image);
            }
            if self.preview_open {
                self.close_preview(window, cx);
            }
        }
        let top = self.history_top(cx);
        let content_list = self.history_list(cx);
        if !quick {
            return div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(top)
                .child(
                    div()
                        .flex_1()
                        .min_h_0()
                        .flex()
                        .child(
                            div()
                                .w(px(if f32::from(window.viewport_size().width) < 1000. {
                                    300.
                                } else {
                                    410.
                                }))
                                .flex_shrink_0()
                                .min_h_0()
                                .flex()
                                .flex_col()
                                .child(content_list),
                        )
                        .child(self.detail(window, cx)),
                )
                .into_any_element();
        }
        let body = div()
            .flex_1()
            .min_h_0()
            .flex()
            .flex_col()
            .child(top)
            .child(content_list);
        if self.preview_open {
            if f32::from(window.viewport_size().width) > QUICK_WIDTH + 100. {
                div()
                    .flex_1()
                    .min_h_0()
                    .flex()
                    .child(
                        div()
                            .w(px(QUICK_WIDTH))
                            .flex_shrink_0()
                            .flex()
                            .flex_col()
                            .child(body),
                    )
                    .child(self.detail(window, cx))
                    .into_any_element()
            } else {
                self.detail(window, cx)
            }
        } else {
            body.into_any_element()
        }
    }

    #[inline(never)]
    fn history_top(&self, cx: &mut Context<Self>) -> AnyElement {
        let quick = self.kind == Kind::Quick;
        let margin = if quick { 14. } else { 24. };
        let mut top = div().flex_shrink_0().flex().flex_col();
        if quick {
            top = top.child(
                div()
                    .px(px(margin))
                    .pt_1()
                    .pb_2()
                    .flex()
                    .items_center()
                    .gap_2()
                    .child(
                        div()
                            .text_size(px(14.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr("剪贴板")),
                    )
                    .child(div().flex_1())
                    .child(status_badge(&self.snapshot, cx))
                    .child(
                        icon_button("quick-manager", IconName::LayoutDashboard, "管理历史")
                            .on_click(|_, _, cx| open_window(Kind::Main, cx)),
                    ),
            );
        } else {
            top = top.child(
                div()
                    .px(px(margin))
                    .pt_4()
                    .pb_4()
                    .flex()
                    .items_center()
                    .justify_between()
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_1()
                            .child(
                                div()
                                    .text_size(px(21.))
                                    .font_weight(FontWeight::SEMIBOLD)
                                    .child(tr(match self.route {
                                        "pinned" => "置顶",
                                        "favorites" => "收藏",
                                        _ => "全部历史",
                                    })),
                            )
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .text_color(cx.theme().muted_foreground)
                                    .child(tr("按最近复制排序，置顶记录始终在前。")),
                            ),
                    )
                    .child(
                        Button::new("open-quick")
                            .small()
                            .h(px(30.))
                            .icon(IconName::Clipboard)
                            .label(tr("打开快捷面板"))
                            .on_click(|_, _, cx| open_window(Kind::Quick, cx)),
                    ),
            );
        }
        top = top.child(
            div()
                .px(px(margin))
                .pb_2()
                .flex()
                .items_center()
                .gap_2()
                .child(
                    div()
                        .flex_1()
                        .min_w_0()
                        .when(!quick, |d| d.max_w(px(445.)))
                        .child(
                            Input::new(&self.search)
                                .cleanable(true)
                                .h(px(34.))
                                .prefix(Icon::new(IconName::Search).size(px(15.))),
                        ),
                )
                .when(!quick, |d| {
                    d.child(div().flex_1())
                        .child(
                            Button::new("multi")
                                .small()
                                .h(px(30.))
                                .icon(IconName::Check)
                                .label(tr(if self.multi { "完成" } else { "多选" }))
                                .on_click(cx.listener(|t, _, _, cx| {
                                    t.multi = !t.multi;
                                    t.checked.clear();
                                    cx.notify();
                                })),
                        )
                        .child(
                            Button::new("clear")
                                .small()
                                .h(px(30.))
                                .icon(IconName::Trash)
                                .label(tr(if self.multi {
                                    "删除所选"
                                } else {
                                    "清空历史"
                                }))
                                .disabled(self.multi && self.checked.is_empty())
                                .on_click(cx.listener(|t, _, w, cx| {
                                    t.confirm = Some(!t.multi);
                                    t.clear_protected = false;
                                    t.dialog_focus.focus(w, cx);
                                    cx.notify();
                                })),
                        )
                }),
        );
        let mut tabs = div()
            .px(px(margin))
            .h(px(36.))
            .flex()
            .items_center()
            .gap(px(if quick { 6. } else { 20. }))
            .border_b_1()
            .border_color(cx.theme().border);
        for (filter, label) in [
            ("favorites", "收藏"),
            ("pinned", "置顶"),
            ("all", if quick { "全部" } else { "全部内容" }),
            ("text", "文本"),
            ("rich", "富文本"),
            ("image", "图片"),
            ("file", "文件"),
        ]
        .into_iter()
        .filter(|(filter, _)| {
            if quick {
                *filter != "rich"
            } else {
                !matches!(*filter, "pinned" | "favorites")
            }
        }) {
            tabs = tabs.child(
                div()
                    .id(SharedString::from(format!("filter-{filter}")))
                    .h_full()
                    .flex()
                    .items_center()
                    .text_size(px(if quick { 11. } else { 12. }))
                    .flex_shrink_0()
                    .cursor_pointer()
                    .border_b_2()
                    .border_color(if self.filter == filter {
                        accent(cx)
                    } else {
                        rgba(0x00000000).into()
                    })
                    .text_color(if self.filter == filter {
                        accent(cx)
                    } else {
                        cx.theme().muted_foreground
                    })
                    .when(self.filter == filter, |d| {
                        d.font_weight(FontWeight::SEMIBOLD)
                    })
                    .child(tr(label).to_owned())
                    .on_click(cx.listener(move |t, _, _, cx| {
                        t.filter = filter;
                        cx.notify();
                    })),
            );
        }
        tabs = tabs.child(div().flex_1());
        if quick {
            tabs = tabs
                .child(
                    icon_button("quick-emoji", IconName::FaceSlightlySmiling, "表情")
                        .on_click(cx.listener(|t, _, _, cx| t.nav("emoji", cx))),
                )
                .child(
                    Button::new("quick-symbols")
                        .ghost()
                        .w(px(24.))
                        .h(px(28.))
                        .p_0()
                        .child(div().text_size(px(16.)).child("Σ"))
                        .tooltip(tr("符号"))
                        .accessibility_label(tr("符号"))
                        .on_click(cx.listener(|t, _, _, cx| t.nav("symbols", cx))),
                );
        } else {
            tabs = tabs.child(
                div()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child(if self.multi {
                        message("已选 {} 条", &[("", self.checked.len().to_string())])
                    } else {
                        tr("最近复制").into()
                    }),
            );
        }
        top = top.child(tabs);
        top.into_any_element()
    }
    #[inline(never)]
    fn history_list(&mut self, cx: &mut Context<Self>) -> AnyElement {
        if !self.snapshot.history_loaded {
            return div()
                .flex_1()
                .flex()
                .items_center()
                .justify_center()
                .text_size(px(12.))
                .text_color(cx.theme().muted_foreground)
                .child(tr("正在加载历史…"))
                .into_any_element();
        }
        let rows = self.visible(cx);
        let entries = self.snapshot.entries.clone();
        let pinned = rows.iter().filter(|index| entries[**index].pinned).count();
        let mut lines = Vec::with_capacity(rows.len() + 2);
        if pinned > 0 {
            lines.push(HistoryLine::Heading("置顶", pinned));
        }
        for (at, index) in rows.iter().enumerate() {
            if at == pinned {
                lines.push(HistoryLine::Heading("最近复制", rows.len() - pinned));
            }
            lines.push(HistoryLine::Entry(*index));
        }
        let signature: Vec<_> = lines
            .iter()
            .map(|line| match line {
                HistoryLine::Heading(label, count) => format!("{label}:{count}"),
                HistoryLine::Entry(index) => {
                    format!("{}:{}", entries[*index].id, self.snapshot.settings.compact)
                }
            })
            .collect();
        if self.history_signature != signature {
            self.history_scroll.reset(lines.len());
            self.history_signature = signature;
        }
        if rows.is_empty() {
            let searching = !self.search.read(cx).value().is_empty();
            let (icon, title, description) = if searching {
                (
                    IconName::Search,
                    "没有找到匹配内容",
                    "试试其他关键词，或清除当前搜索。",
                )
            } else {
                match if self.kind == Kind::Quick {
                    self.filter
                } else {
                    self.route
                } {
                    "pinned" => (
                        IconName::Pin,
                        "还没有置顶内容",
                        "点击记录上的图钉，将常用内容留在顶部。",
                    ),
                    "favorites" => (
                        IconName::Star,
                        "还没有收藏内容",
                        "点击星标，收藏值得保留的内容。",
                    ),
                    _ => (
                        IconName::Clipboard,
                        "复制的内容，会出现在这里",
                        "复制文字、图片或文件，开始你的第一条记录。",
                    ),
                }
            };
            div()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .items_center()
                .justify_center()
                .px_5()
                .gap_3()
                .child(
                    div()
                        .size(px(62.))
                        .rounded(px(18.))
                        .bg(cx.theme().muted)
                        .flex()
                        .items_center()
                        .justify_center()
                        .child(
                            Icon::new(icon)
                                .size(px(28.))
                                .text_color(cx.theme().muted_foreground),
                        ),
                )
                .child(
                    div()
                        .text_size(px(15.))
                        .font_weight(FontWeight::MEDIUM)
                        .child(tr(title)),
                )
                .child(
                    div()
                        .text_size(px(12.))
                        .text_color(cx.theme().muted_foreground)
                        .text_center()
                        .child(tr(description)),
                )
                .when(searching, |d| {
                    d.child(
                        Button::new("clear-search")
                            .small()
                            .h(px(30.))
                            .label(tr("清除搜索"))
                            .on_click(cx.listener(|t, _, w, cx| {
                                t.search.update(cx, |input, cx| input.set_value("", w, cx))
                            })),
                    )
                })
                .into_any_element()
        } else {
            let view = cx.entity().downgrade();
            let rows = list(self.history_scroll.clone(), move |index, w, cx| {
                view.update(cx, |t, cx| match &lines[index] {
                    HistoryLine::Heading(label, count) => div()
                        .w_full()
                        .h(px(28.))
                        .pl_3()
                        .pr(px(24.))
                        .flex()
                        .items_center()
                        .gap_1()
                        .text_size(px(11.))
                        .text_color(cx.theme().muted_foreground)
                        .when(*label == "置顶", |d| {
                            d.child(Icon::new(IconName::Pin).size(px(12.)))
                        })
                        .child(tr(label))
                        .child(div().flex_1())
                        .child(count.to_string())
                        .into_any_element(),
                    HistoryLine::Entry(i) => div()
                        .px_2()
                        .pb(px(3.))
                        .child(t.row(entries[*i].clone(), w, cx))
                        .into_any_element(),
                })
                .unwrap_or_else(|_| div().into_any_element())
            })
            .flex_1()
            .min_h_0()
            .py_1();
            div()
                .relative()
                .flex_1()
                .min_h_0()
                .flex()
                .flex_col()
                .child(rows)
                .child(
                    gpui_kit::component::scroll::Scrollbar::vertical(&self.history_scroll)
                        .mode(gpui_kit::component::scroll::ScrollbarMode::Always),
                )
                .into_any_element()
        }
    }

    #[inline(never)]
    fn row(&mut self, entry: Entry, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let image_content = entry.kind == "image";
        if image_content {
            self.service.thumbnail(&entry.id);
            if !self.thumbnails.contains_key(&entry.id)
                && let Some(Some(pixels)) = self.snapshot.thumbnails.get(&entry.id)
            {
                let mut pixels = pixels.as_ref().clone();
                for pixel in pixels.chunks_exact_mut(4) {
                    pixel.swap(0, 2);
                }
                self.thumbnails.insert(
                    entry.id.clone(),
                    Arc::new(RenderImage::new(vec![image::Frame::new(pixels)])),
                );
            }
        }
        let title = if image_content {
            div()
                .h(px(40.))
                .flex()
                .items_center()
                .child(if let Some(image) = self.thumbnails.get(&entry.id) {
                    img(image.clone())
                        .max_w(px(96.))
                        .h(px(40.))
                        .object_fit(ObjectFit::Contain)
                        .into_any_element()
                } else {
                    div()
                        .text_size(px(11.))
                        .text_color(cx.theme().muted_foreground)
                        .child(tr(if self.snapshot.thumbnails.contains_key(&entry.id) {
                            "缩略图不可用"
                        } else {
                            "正在加载缩略图…"
                        }))
                        .into_any_element()
                })
                .into_any_element()
        } else {
            div()
                .truncate()
                .child(record_title(&entry))
                .into_any_element()
        };
        let selected = self.selected.as_deref() == Some(&entry.id);
        let compact = self.snapshot.settings.compact;
        let quick = self.kind == Kind::Quick;
        let id = entry.id.clone();
        let copy_id = id.clone();
        let preview_id = id.clone();
        let pin_id = id.clone();
        let fav_id = id.clone();
        let more_id = id.clone();
        let mut description = entry
            .text
            .lines()
            .skip(1)
            .find(|line| !line.trim().is_empty())
            .unwrap_or("")
            .to_owned();
        if description.is_empty() {
            description = if entry.kind == "file" {
                tr("仅保存文件引用，原文件仍在原位置").into()
            } else if entry.kind == "image" {
                message("图片 · {}", &[("", bytes_label(entry.bytes))])
            } else {
                format!(
                    "{} · {}",
                    tr(kind_label(&entry.kind)),
                    bytes_label(entry.bytes)
                )
            };
        }
        let menu = self.row_menu(&entry, cx);
        div()
            .id(SharedString::from(id.clone()))
            .w_full()
            .min_w_0()
            .h(px(if image_content {
                if compact { 84. } else { 104. }
            } else if compact {
                68.
            } else {
                84.
            }))
            .px(px(10.))
            .py(px(8.))
            .rounded(px(7.))
            .bg(if selected {
                selected_color(cx)
            } else {
                cx.theme().background
            })
            .border_l_2()
            .border_color(if selected {
                accent(cx)
            } else {
                rgba(0x00000000).into()
            })
            .hover(|d| {
                d.bg(if selected {
                    selected_color(cx)
                } else {
                    cx.theme().muted
                })
            })
            .on_click(cx.listener(move |t, event: &ClickEvent, w, cx| {
                t.select(id.clone(), w, cx);
                if event.click_count() == 2 && !t.multi {
                    t.write(id.clone(), false, w, cx);
                }
            }))
            .child(
                div()
                    .flex()
                    .items_start()
                    .gap(px(10.))
                    .child(type_mark(
                        &entry,
                        self.snapshot
                            .preview
                            .as_ref()
                            .is_some_and(|preview| preview.id == entry.id && preview.is_directory),
                        cx,
                    ))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .flex()
                            .flex_col()
                            .gap(px(2.))
                            .child(
                                div()
                                    .flex()
                                    .items_center()
                                    .gap_2()
                                    .when(self.multi, |d| {
                                        d.child(
                                            Icon::new(if self.checked.contains(&entry.id) {
                                                IconName::Check
                                            } else {
                                                IconName::Square
                                            })
                                            .size(px(14.))
                                            .text_color(accent(cx)),
                                        )
                                    })
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .truncate()
                                            .text_size(px(13.))
                                            .font_weight(FontWeight::MEDIUM)
                                            .child(title),
                                    )
                                    .child(
                                        div()
                                            .text_size(px(10.))
                                            .text_color(cx.theme().muted_foreground)
                                            .flex_shrink_0()
                                            .child(age(entry.copied_at)),
                                    ),
                            )
                            .when(!compact && !image_content, |d| {
                                d.child(
                                    div()
                                        .truncate()
                                        .text_size(px(11.))
                                        .text_color(cx.theme().muted_foreground)
                                        .child(description),
                                )
                            }),
                    ),
            )
            .child(
                div()
                    .ml(px(43.))
                    .mt(px(if compact { 0. } else { 3. }))
                    .h(px(26.))
                    .flex()
                    .items_center()
                    .gap(px(1.))
                    .child(
                        div()
                            .flex_1()
                            .min_w_0()
                            .truncate()
                            .text_size(px(10.))
                            .text_color(cx.theme().muted_foreground)
                            .child(format!(
                                "{} · {}",
                                entry.source,
                                tr(kind_label(&entry.kind))
                            )),
                    )
                    .child(
                        icon_button(
                            "copy",
                            if quick {
                                IconName::ClipboardPaste
                            } else {
                                IconName::Copy
                            },
                            if quick { "粘贴" } else { "复制" },
                        )
                        .on_click(cx.listener(move |t, _, w, cx| {
                            cx.stop_propagation();
                            t.write(copy_id.clone(), false, w, cx);
                        })),
                    )
                    .child(
                        icon_button("preview", IconName::Eye, "预览").on_click(cx.listener(
                            move |t, _, w, cx| {
                                cx.stop_propagation();
                                t.open_preview(preview_id.clone(), w, cx);
                            },
                        )),
                    )
                    .child(
                        icon_button(
                            "pin",
                            if entry.pinned {
                                IconName::PinOff
                            } else {
                                IconName::Pin
                            },
                            if entry.pinned {
                                "取消置顶"
                            } else {
                                "置顶"
                            },
                        )
                        .when(entry.pinned, |b| {
                            b.bg(accent_soft(cx)).text_color(accent(cx))
                        })
                        .on_click(cx.listener(move |t, _, w, cx| {
                            cx.stop_propagation();
                            t.service.send(Command::Protect(
                                pin_id.clone(),
                                Some(!entry.pinned),
                                None,
                            ));
                            t.sync(w, cx);
                        })),
                    )
                    .child(
                        icon_button(
                            "favorite",
                            if entry.favorite {
                                IconName::StarOff
                            } else {
                                IconName::Star
                            },
                            if entry.favorite {
                                "取消收藏"
                            } else {
                                "收藏"
                            },
                        )
                        .when(entry.favorite, |b| {
                            b.bg(warning_soft(cx)).text_color(warning(cx))
                        })
                        .on_click(cx.listener(move |t, _, w, cx| {
                            cx.stop_propagation();
                            t.service.send(Command::Protect(
                                fav_id.clone(),
                                None,
                                Some(!entry.favorite),
                            ));
                            t.sync(w, cx);
                        })),
                    )
                    .child(
                        Popover::new("row-more")
                            .anchor(Anchor::TopRight)
                            .open(self.more.as_deref() == Some(&entry.id))
                            .on_open_change(cx.listener(move |t, open: &bool, _, cx| {
                                t.more = if *open { Some(more_id.clone()) } else { None };
                                cx.notify();
                            }))
                            .trigger(
                                Button::new("more")
                                    .ghost()
                                    .w(px(24.))
                                    .h(px(26.))
                                    .p_0()
                                    .label("···")
                                    .tooltip(tr("更多操作"))
                                    .accessibility_label(tr("更多操作"))
                                    .on_click(|_, _, cx| cx.stop_propagation()),
                            )
                            .child(menu),
                    ),
            )
            .into_any_element()
    }
    #[inline(never)]
    fn row_menu(&self, entry: &Entry, cx: &mut Context<Self>) -> AnyElement {
        let quick = self.kind == Kind::Quick;
        let plain_id = entry.id.clone();
        let delete_id = entry.id.clone();
        div()
            .w(px(176.))
            .p_1()
            .flex()
            .flex_col()
            .gap_1()
            .child(
                menu_action(
                    "plain-action",
                    if quick {
                        "纯文本粘贴"
                    } else {
                        "复制纯文本"
                    },
                    IconName::FileText,
                    None,
                    cx,
                )
                .disabled(entry.text.is_empty())
                .on_click(cx.listener(move |t, _, w, cx| {
                    cx.stop_propagation();
                    t.more = None;
                    t.write(plain_id.clone(), true, w, cx);
                    cx.notify();
                })),
            )
            .child(div().h(px(1.)).my_1().bg(cx.theme().border))
            .child(
                menu_action("delete-action", "删除记录…", IconName::Trash, None, cx)
                    .text_color(danger(cx))
                    .on_click(cx.listener(move |t, _, w, cx| {
                        cx.stop_propagation();
                        t.more = None;
                        t.checked.clear();
                        t.checked.insert(delete_id.clone());
                        t.confirm = Some(false);
                        t.dialog_focus.focus(w, cx);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}

fn type_mark_kind(entry: &Entry, directory: bool) -> &str {
    if entry.kind != "file" {
        return &entry.kind;
    }
    if entry
        .title
        .rsplit_once(" 等 ")
        .and_then(|(_, tail)| tail.strip_suffix(" 个项目"))
        .and_then(|count| count.parse::<usize>().ok())
        .is_some_and(|count| count > 1)
    {
        return "files";
    }
    if directory {
        return "folder";
    }
    let ext = std::path::Path::new(&entry.title)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match ext.as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tif" | "tiff" | "ico" | "svg"
        | "avif" | "heic" | "heif" => "image",
        "pdf" => "pdf",
        "rs" | "js" | "ts" | "tsx" | "jsx" | "json" | "md" | "toml" | "yaml" | "yml" | "xml"
        | "html" | "css" | "py" | "ini" | "sh" | "sql" | "c" | "cpp" | "h" => "code",
        "xlsx" | "xls" | "xlsm" | "ods" | "csv" => "spreadsheet",
        "lnk" | "url" | "desktop" => "shortcut",
        _ => "file",
    }
}
#[inline(never)]
fn type_mark(entry: &Entry, directory: bool, cx: &App) -> AnyElement {
    let kind = type_mark_kind(entry, directory);
    let (icon, label, color, bg) = match kind {
        "image" | "spreadsheet" => (
            Some(if kind == "image" {
                IconName::Image
            } else {
                IconName::Table2
            }),
            "",
            rgb(if cx.theme().is_dark() {
                0x92c4a2
            } else {
                0x4c7c5e
            })
            .into(),
            rgb(if cx.theme().is_dark() {
                0x293e32
            } else {
                0xe3eee6
            })
            .into(),
        ),
        "rich" => (None, "B", cx.theme().muted_foreground, cx.theme().muted),
        "pdf" => (
            Some(IconName::FileText),
            "",
            danger(cx),
            danger(cx).opacity(0.1),
        ),
        "code" => (
            None,
            "</>",
            rgb(if cx.theme().is_dark() {
                0x98baff
            } else {
                0x356bc4
            })
            .into(),
            rgb(if cx.theme().is_dark() {
                0x2c3b51
            } else {
                0xeaf0fc
            })
            .into(),
        ),
        "folder" => (Some(IconName::Folder), "", warning(cx), warning_soft(cx)),
        "files" => (Some(IconName::Files), "", warning(cx), warning_soft(cx)),
        "shortcut" => (
            Some(IconName::FileSymlink),
            "",
            cx.theme().muted_foreground,
            cx.theme().muted,
        ),
        "file" => (
            Some(IconName::File),
            "",
            cx.theme().muted_foreground,
            cx.theme().muted,
        ),
        _ => (None, "T", cx.theme().muted_foreground, cx.theme().muted),
    };
    div()
        .size(px(32.))
        .flex_shrink_0()
        .rounded(px(8.))
        .border_1()
        .border_color(cx.theme().border.opacity(0.65))
        .bg(bg)
        .text_color(color)
        .flex()
        .items_center()
        .justify_center()
        .when_some(icon, |d, icon| d.child(Icon::new(icon).size(px(18.))))
        .when(icon.is_none(), |d| {
            d.child(
                div()
                    .text_size(px(if label == "</>" { 13. } else { 19. }))
                    .child(tr(label).to_owned()),
            )
        })
        .into_any_element()
}

#[cfg(test)]
mod tests {
    use super::{Entry, type_mark_kind};
    fn file_entry(title: &str) -> Entry {
        Entry {
            id: "file-reference".into(),
            fingerprint: String::new(),
            title: title.into(),
            text: String::new(),
            kind: "file".into(),
            source: "explorer.exe".into(),
            copied_at: 0,
            order: 0,
            pinned: false,
            favorite: false,
            bytes: 0,
        }
    }
    #[test]
    fn image_file_reference_has_image_mark_and_keeps_file_semantics() {
        let entry = file_entry("26-09-周报.png");
        assert_eq!(type_mark_kind(&entry, false), "image");
        assert_eq!(entry.kind, "file");
        assert_eq!(type_mark_kind(&file_entry("PHOTO.PNG"), false), "image");
    }
    #[test]
    fn only_confirmed_directories_get_folder_marks() {
        assert_eq!(
            type_mark_kind(&file_entry("26-09-周报.png"), true),
            "folder"
        );
        assert_eq!(type_mark_kind(&file_entry("README"), false), "file");
        assert_eq!(
            type_mark_kind(&file_entry("DeepSeek Harness.lnk"), false),
            "shortcut"
        );
        assert_eq!(
            type_mark_kind(&file_entry("钉钉订阅汇总表_26.xlsx"), false),
            "spreadsheet"
        );
        assert_eq!(type_mark_kind(&file_entry("report.PDF"), false), "pdf");
        assert_eq!(type_mark_kind(&file_entry("main.RS"), false), "code");
        assert_eq!(
            type_mark_kind(&file_entry("image.png 等 3 个项目"), false),
            "files"
        );
    }
}
