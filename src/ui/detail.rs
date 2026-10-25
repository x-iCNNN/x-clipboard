use super::*;
use x_clipboard::preview::Preview;

impl Desktop {
    #[inline(never)]
    pub(super) fn detail(&mut self, _: &mut Window, cx: &mut Context<Self>) -> AnyElement {
        let mut detail = div()
            .flex_1()
            .min_w_0()
            .h_full()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .flex()
            .flex_col();
        let Some(id) = self.selected.clone() else {
            return detail.child(Self::detail_empty(cx)).into_any_element();
        };
        let preview = self.snapshot.preview.clone().filter(|p| p.id == id);
        let entry = self.snapshot.entries.iter().find(|e| e.id == id).cloned();
        let type_name = detail_type_name(entry.as_ref(), preview.as_deref());
        detail = detail.child(self.detail_heading(type_name, cx));
        if let Some(entry) = &entry {
            detail = detail.child(Self::detail_metadata(entry, cx));
        }
        if let Some(preview) = preview {
            if let Some(error) = &preview.error {
                detail = detail.child(Self::detail_error(error, cx));
            }
            if let Some(image) = self.image.clone() {
                detail = detail.child(self.detail_image(image, cx));
                detail = detail.child(self.detail_image_toolbar(
                    id.clone(),
                    preview.page,
                    preview.pages,
                    cx,
                ));
            } else if let Some(markup) = &preview.markup {
                detail = detail.child(Self::detail_rich_text(markup));
            } else {
                detail = detail.child(self.detail_plain_text(type_name == "代码", cx));
            }
            detail = detail.child(Self::detail_formats(&preview, cx));
            if let Some(file) = preview.files.first() {
                detail = detail.child(self.detail_open_file(file.clone(), cx));
            }
        } else {
            detail = detail.child(Self::detail_loading(cx));
        }
        detail
            .child(self.detail_actions(id, entry, cx))
            .into_any_element()
    }

    #[inline(never)]
    fn detail_empty(cx: &App) -> AnyElement {
        div()
            .size_full()
            .flex()
            .flex_col()
            .items_center()
            .justify_center()
            .gap_3()
            .text_color(cx.theme().muted_foreground)
            .child(
                div()
                    .size(px(58.))
                    .rounded(px(16.))
                    .bg(cx.theme().muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(Icon::new(IconName::Eye).size(px(26.))),
            )
            .child(div().text_size(px(14.)).child(tr("选择一条记录查看详情")))
            .child(
                div()
                    .text_size(px(11.))
                    .child(tr("文字可选择复制，文件始终保留原始引用。")),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn detail_heading(&self, type_name: &str, cx: &mut Context<Self>) -> AnyElement {
        div()
            .px_4()
            .h(px(42.))
            .flex_shrink_0()
            .border_b_1()
            .border_color(cx.theme().border)
            .flex()
            .items_center()
            .gap_2()
            .when(self.kind == Kind::Quick, |d| {
                d.child(
                    icon_button("back-preview", IconName::ChevronLeft, "返回记录")
                        .on_click(cx.listener(|t, _, w, cx| t.close_preview(w, cx))),
                )
            })
            .child(
                Icon::new(match type_name {
                    "图片" => IconName::Image,
                    "文件" | "文件夹" => IconName::Folder,
                    "富文本" => IconName::Bold,
                    "代码" => IconName::CodeXml,
                    "Markdown" => IconName::BookOpen,
                    _ => IconName::FileText,
                })
                .size(px(17.))
                .text_color(cx.theme().muted_foreground),
            )
            .child(
                div()
                    .flex_1()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .font_weight(FontWeight::MEDIUM)
                    .truncate()
                    .child(message("{type_name}预览", &[("type_name", tr(type_name).into())])),
            )
            .child(
                div()
                    .text_size(px(10.))
                    .px_2()
                    .py_1()
                    .rounded(px(4.))
                    .bg(cx.theme().muted)
                    .text_color(cx.theme().muted_foreground)
                    .child(tr("本地内容")),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn detail_metadata(entry: &Entry, cx: &App) -> AnyElement {
        div()
            .px_5()
            .pt_4()
            .pb_3()
            .flex_shrink_0()
            .flex()
            .flex_col()
            .gap_2()
            .child(
                div()
                    .text_size(px(17.))
                    .font_weight(FontWeight::SEMIBOLD)
                    .truncate()
                    .child(record_title(entry)),
            )
            .child(
                div()
                    .flex()
                    .flex_wrap()
                    .items_center()
                    .gap_2()
                    .text_size(px(11.))
                    .text_color(cx.theme().muted_foreground)
                    .child(format!("{} · {}", entry.source, age(entry.copied_at)))
                    .when(entry.pinned, |d| {
                        d.child(
                            div()
                                .px_1()
                                .rounded(px(3.))
                                .bg(accent_soft(cx))
                                .text_color(accent(cx))
                                .child(tr("已置顶")),
                        )
                    })
                    .when(entry.favorite, |d| {
                        d.child(
                            div()
                                .px_1()
                                .rounded(px(3.))
                                .bg(warning_soft(cx))
                                .text_color(warning(cx))
                                .child(tr("已收藏")),
                        )
                    }),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn detail_error(error: &str, cx: &App) -> AnyElement {
        div()
            .mx_4()
            .mb_3()
            .p_3()
            .rounded(px(7.))
            .bg(warning_soft(cx))
            .text_size(px(12.))
            .text_color(warning(cx))
            .child(error.to_owned())
            .into_any_element()
    }

    #[inline(never)]
    fn detail_image(&self, image: Arc<RenderImage>, cx: &App) -> AnyElement {
        let image_element = if self.zoom == 0. {
            canvas(
                |_, _, _| (),
                move |bounds, _, window, _| {
                    let source = image.size(0);
                    let width = (f32::from(bounds.size.width) - 24.).max(0.);
                    let height = (f32::from(bounds.size.height) - 24.).max(0.);
                    let scale =
                        (width / source.width.0 as f32).min(height / source.height.0 as f32);
                    let fitted = size(
                        px(source.width.0 as f32 * scale),
                        px(source.height.0 as f32 * scale),
                    );
                    let image_bounds = Bounds::new(
                        point(
                            bounds.origin.x + (bounds.size.width - fitted.width) / 2.,
                            bounds.origin.y + (bounds.size.height - fitted.height) / 2.,
                        ),
                        fitted,
                    );
                    let _ = window.paint_image(
                        bounds,
                        image_bounds,
                        Corners::default(),
                        image,
                        0,
                        false,
                    );
                },
            )
            .absolute()
            .inset_0()
            .into_any_element()
        } else {
            let size = image.size(0);
            img(image)
                .w(px(size.width.0 as f32 * self.zoom))
                .h(px(size.height.0 as f32 * self.zoom))
                .into_any_element()
        };
        div()
            .id("image-scroll")
            .relative()
            .flex_1()
            .min_h_0()
            .when(self.zoom == 0., |d| d.overflow_hidden())
            .when(self.zoom != 0., |d| d.overflow_scroll().p_3())
            .mx_4()
            .mb_2()
            .rounded(px(7.))
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .child(image_element)
            .into_any_element()
    }

    #[inline(never)]
    fn detail_image_toolbar(
        &self,
        id: String,
        page: u16,
        pages: u16,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let mut toolbar = div()
            .px_3()
            .py_2()
            .flex_shrink_0()
            .flex()
            .flex_wrap()
            .items_center()
            .gap_2()
            .child(self.detail_zoom_control("fit", "适应", |_| 0., cx))
            .child(self.detail_zoom_control("actual", "1:1", |_| 1., cx))
            .child(self.detail_zoom_control(
                "zoom-out",
                "−",
                |zoom| (if zoom == 0. { 1. } else { zoom } / 1.25).max(0.1),
                cx,
            ))
            .child(self.detail_zoom_control(
                "zoom-in",
                "+",
                |zoom| (if zoom == 0. { 1. } else { zoom } * 1.25).min(4.),
                cx,
            ));
        if pages > 0 {
            toolbar = toolbar.child(self.detail_page_controls(id, page, pages, cx));
        }
        toolbar.into_any_element()
    }

    #[inline(never)]
    fn detail_zoom_control(
        &self,
        id: &'static str,
        label: &'static str,
        update: fn(f32) -> f32,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        Button::new(id)
            .small()
            .h(px(30.))
            .label(tr(label).to_owned())
            .on_click(cx.listener(move |t, _, _, cx| {
                t.zoom = update(t.zoom);
                cx.notify();
            }))
            .into_any_element()
    }

    #[inline(never)]
    fn detail_page_controls(
        &self,
        id: String,
        page: u16,
        pages: u16,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let prev_id = id.clone();
        div()
            .flex()
            .flex_shrink_0()
            .items_center()
            .gap_2()
            .child(
                Button::new("prev-page")
                    .small()
                    .h(px(30.))
                    .label("‹")
                    .disabled(page == 0)
                    .on_click(cx.listener(move |t, _, _, _| {
                        t.preview_generation =
                            t.service.preview(prev_id.clone(), page.saturating_sub(1));
                    })),
            )
            .child(format!("{}/{}", page + 1, pages))
            .child(
                Button::new("next-page")
                    .small()
                    .h(px(30.))
                    .label("›")
                    .disabled(page + 1 >= pages)
                    .on_click(cx.listener(move |t, _, _, _| {
                        t.preview_generation = t.service.preview(id.clone(), page + 1);
                    })),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn detail_rich_text(markup: &str) -> AnyElement {
        div()
            .id("rich-preview")
            .flex_1()
            .min_h_0()
            .overflow_y_scroll()
            .px_5()
            .pb_4()
            .text_size(px(13.))
            .child(gpui_kit::component::text::TextView::html(
                "markup-content",
                markup.to_owned(),
            ))
            .into_any_element()
    }

    #[inline(never)]
    fn detail_plain_text(&self, code: bool, cx: &App) -> AnyElement {
        div()
            .flex_1()
            .min_h_0()
            .mx_4()
            .when(code, |d| {
                d.p_2()
                    .rounded(px(7.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().muted)
            })
            .child(
                Textarea::new(&self.editor)
                    .readonly(true)
                    .appearance(false)
                    .when(code, |t| {
                        t.font_family(if cfg!(windows) {
                            "Consolas"
                        } else {
                            "monospace"
                        })
                        .text_size(px(12.))
                    })
                    .h_full(),
            )
            .into_any_element()
    }

    #[inline(never)]
    fn detail_formats(preview: &Preview, cx: &App) -> AnyElement {
        div()
            .px_4()
            .py_2()
            .text_size(px(10.))
            .text_color(cx.theme().muted_foreground)
            .child(message(
                "保留格式：{}",
                &[("", preview
                    .formats
                    .iter()
                    .map(|f| tr(format_label(f)))
                    .collect::<Vec<_>>()
                    .join(" · "))]
            ))
            .into_any_element()
    }

    #[inline(never)]
    fn detail_open_file(&self, file: String, cx: &mut Context<Self>) -> AnyElement {
        Button::new("open-file")
            .ghost()
            .label(tr("使用默认应用打开"))
            .on_click(cx.listener(move |this, _, _, _| {
                let file = file.clone();
                let service = this.service.clone();
                std::thread::spawn(move || {
                    if let Err(e) = open::that(file) {
                        service.error(format!("无法打开文件：{e}"));
                    }
                });
            }))
            .into_any_element()
    }

    #[inline(never)]
    fn detail_loading(cx: &App) -> AnyElement {
        div()
            .flex_1()
            .p_5()
            .flex()
            .flex_col()
            .gap_3()
            .child(
                div()
                    .text_size(px(12.))
                    .text_color(cx.theme().muted_foreground)
                    .child(tr("正在加载预览…")),
            )
            .children([0.9, 0.75, 0.95, 0.55].into_iter().map(|width| {
                div()
                    .h(px(12.))
                    .w(relative(width))
                    .rounded(px(4.))
                    .bg(cx.theme().muted)
            }))
            .into_any_element()
    }

    #[inline(never)]
    fn detail_actions(
        &self,
        id: String,
        entry: Option<Entry>,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let plain_enabled = entry.as_ref().is_some_and(|e| !e.text.is_empty());
        let mut actions = div()
            .p_3()
            .border_t_1()
            .border_color(cx.theme().border)
            .flex()
            .gap_2()
            .child(self.detail_copy_action(id.clone(), cx))
            .child(self.detail_plain_action(id, plain_enabled, cx))
            .child(div().flex_1());
        if let Some(entry) = entry {
            actions = actions.child(self.detail_favorite_action(entry, cx));
        }
        actions.into_any_element()
    }

    #[inline(never)]
    fn detail_copy_action(&self, id: String, cx: &mut Context<Self>) -> AnyElement {
        Button::new("detail-copy")
            .primary()
            .small()
            .h(px(30.))
            .icon(IconName::Copy)
            .label(tr(if self.kind == Kind::Quick {
                "粘贴内容"
            } else {
                "复制内容"
            }))
            .on_click(cx.listener(move |t, _, w, cx| t.write(id.clone(), false, w, cx)))
            .into_any_element()
    }

    #[inline(never)]
    fn detail_plain_action(&self, id: String, enabled: bool, cx: &mut Context<Self>) -> AnyElement {
        Button::new("detail-plain")
            .small()
            .h(px(30.))
            .icon(IconName::FileText)
            .label(tr("纯文本"))
            .disabled(!enabled)
            .on_click(cx.listener(move |t, _, w, cx| t.write(id.clone(), true, w, cx)))
            .into_any_element()
    }

    #[inline(never)]
    fn detail_favorite_action(&self, entry: Entry, cx: &mut Context<Self>) -> AnyElement {
        icon_button(
            "detail-favorite",
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
            t.service.send(Command::Protect(
                entry.id.clone(),
                None,
                Some(!entry.favorite),
            ));
            t.sync(w, cx);
        }))
        .into_any_element()
    }
}

fn detail_type_name(entry: Option<&Entry>, preview: Option<&Preview>) -> &'static str {
    let Some(entry) = entry else {
        return "内容";
    };
    if entry.kind != "file" {
        return kind_label(&entry.kind);
    }
    if preview.is_some_and(|p| p.is_directory) {
        return "文件夹";
    }
    let path = preview
        .and_then(|p| p.files.first())
        .map(String::as_str)
        .unwrap_or(&entry.title);
    let extension = std::path::Path::new(path)
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    match extension.as_str() {
        "pdf" => "PDF",
        "md" => "Markdown",
        "rs" | "js" | "ts" | "tsx" | "jsx" | "json" | "toml" | "yaml" | "yml" | "xml" | "html"
        | "css" | "py" | "ini" | "sh" | "sql" | "c" | "cpp" | "h" => "代码",
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "tiff" => "图片",
        "txt" | "log" | "csv" => "文本",
        _ => "文件",
    }
}

fn format_label(format: &str) -> &str {
    match format {
        "windows:13" | "text/plain" => "文本",
        "windows:HTML Format" | "text/html" => "HTML",
        "windows:Rich Text Format" | "text/rtf" => "RTF",
        "windows:PNG" | "image/png" => "PNG",
        "windows:8" | "windows:17" => "位图",
        "windows:15" | "text/uri-list" => "文件引用",
        _ => "原始格式",
    }
}
