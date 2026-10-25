use super::*;
use gpui_kit::component::checkbox::Checkbox;

impl Desktop {
    pub(super) fn confirmation(&self, cx: &mut Context<Self>) -> AnyElement {
        let clear = self.confirm == Some(true);
        let protected = self
            .snapshot
            .entries
            .iter()
            .filter(|e| (clear || self.checked.contains(&e.id)) && (e.pinned || e.favorite))
            .count();
        let ordinary = self
            .snapshot
            .entries
            .iter()
            .filter(|e| !e.pinned && !e.favorite)
            .count();
        let count = if clear {
            if self.clear_protected {
                self.snapshot.entries.len()
            } else {
                ordinary
            }
        } else {
            self.checked.len()
        };
        div()
            .absolute()
            .inset_0()
            .bg(rgba(0x121a244d))
            .flex()
            .items_center()
            .justify_center()
            .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
            .child(
                div()
                    .w(px(if self.kind == Kind::Quick { 360. } else { 400. }))
                    .max_w(relative(0.95))
                    .p_6()
                    .rounded(px(13.))
                    .border_1()
                    .border_color(cx.theme().border)
                    .bg(cx.theme().background)
                    .shadow_lg()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .track_focus(&self.dialog_focus)
                    .child(
                        div()
                            .size(px(43.))
                            .rounded(px(12.))
                            .bg(danger(cx).opacity(0.1))
                            .text_color(danger(cx))
                            .flex()
                            .items_center()
                            .justify_center()
                            .child(Icon::new(IconName::Trash).size(px(22.))),
                    )
                    .child(
                        div()
                            .text_size(px(19.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(tr(if clear {
                                "清空剪贴板历史？"
                            } else {
                                "删除所选记录？"
                            })),
                    )
                    .child(
                        div()
                            .text_size(px(12.))
                            .text_color(cx.theme().muted_foreground)
                            .child(message("将删除 {count} 条历史记录，此操作无法撤销。原始文件不会被删除。", &[("count", count.to_string())])),
                    )
                    .when(clear, |d| {
                        d.child(
                            Checkbox::new("include-protected")
                                .label(tr("同时删除置顶与收藏"))
                                .checked(self.clear_protected)
                                .on_click(cx.listener(|t, checked: &bool, _, cx| {
                                    t.clear_protected = *checked;
                                    cx.notify();
                                })),
                        )
                    })
                    .when(protected > 0, |d| {
                        d.child(
                            div()
                                .p_3()
                                .rounded(px(7.))
                                .bg(warning_soft(cx))
                                .text_size(px(12.))
                                .text_color(warning(cx))
                                .child(if clear && !self.clear_protected {
                                    message("保留 {protected} 条置顶或收藏记录。", &[("protected", protected.to_string())])
                                } else {
                                    message("其中包含 {protected} 条置顶或收藏记录，请确认后再删除。", &[("protected", protected.to_string())])
                                }),
                        )
                    })
                    .child(
                        div()
                            .flex()
                            .justify_end()
                            .items_center()
                            .gap_2()
                            .pt_1()
                            .child(Button::new("cancel-delete").label(tr("取消")).on_click(
                                cx.listener(|t, _, w, cx| {
                                    t.confirm = None;
                                    t.focus.focus(w, cx);
                                    cx.notify();
                                }),
                            ))
                            .child(
                                Button::new("confirm-delete")
                                    .danger()
                                    .label(tr(if clear {
                                        "清空历史"
                                    } else {
                                        "删除记录"
                                    }))
                                    .disabled(count == 0)
                                    .on_click(cx.listener(move |t, _, w, cx| {
                                        if clear {
                                            t.service.send(Command::Clear(t.clear_protected));
                                        } else {
                                            t.service.send(Command::Delete(
                                                t.checked.iter().cloned().collect(),
                                            ));
                                        }
                                        t.checked.clear();
                                        t.confirm = None;
                                        t.focus.focus(w, cx);
                                        cx.notify();
                                    })),
                            ),
                    ),
            )
            .into_any_element()
    }
}
