use super::*;
pub(super) const ACTIONS: [&str; 5] = ["quick", "manager", "pause", "settings", "quit"];
impl Desktop {
    #[inline(never)]
    pub(super) fn tray_panel(&self, cx: &mut Context<Self>) -> AnyElement {
        let paused = self.snapshot.settings.paused;
        let rows = [
            ("打开快捷面板", IconName::Clipboard),
            ("管理历史", IconName::LayoutGrid),
            (
                if paused {
                    "开启记录"
                } else {
                    "暂停记录"
                },
                if paused {
                    IconName::Play
                } else {
                    IconName::Pause
                },
            ),
            ("设置", IconName::Settings),
            ("退出 X Clipboard", IconName::Power),
        ];
        let mut menu = div()
            .size_full()
            .p(px(4.))
            .border_1()
            .border_color(cx.theme().border)
            .bg(self.surface(cx.theme().popover))
            .text_color(cx.theme().popover_foreground)
            .flex()
            .flex_col();
        for (index, (label, icon)) in rows.into_iter().enumerate() {
            if index == 2 || index == 4 {
                menu = menu.child(
                    div()
                        .h(px(9.))
                        .flex_shrink_0()
                        .flex()
                        .items_center()
                        .child(div().w_full().h(px(1.)).bg(cx.theme().border)),
                );
            }
            let action = ACTIONS[index];
            menu =
                menu.child(
                    div()
                        .id(SharedString::from(format!("tray-{action}")))
                        .h(px(34.))
                        .flex_shrink_0()
                        .px_2()
                        .rounded(px(4.))
                        .flex()
                        .items_center()
                        .gap_2()
                        .cursor_pointer()
                        .bg(if self.tray_selected == Some(index) {
                            cx.theme().list_active
                        } else {
                            cx.theme().popover
                        })
                        .on_mouse_move(cx.listener(move |this, _, _, cx| {
                            if this.tray_selected != Some(index) {
                                this.tray_selected = Some(index);
                                cx.notify();
                            }
                        }))
                        .on_click(cx.listener(move |this, _, window, cx| {
                            this.tray_action(action, window, cx)
                        }))
                        .child(Icon::new(icon).size(px(16.)).text_color(accent(cx)))
                        .child(div().text_size(px(13.)).child(tr(label).to_owned()))
                        .child(div().flex_1())
                        .when(index == 0, |row| {
                            row.child(keycap(self.snapshot.settings.shortcut.clone(), cx))
                        }),
                );
        }
        menu.into_any_element()
    }
    pub(super) fn tray_action(
        &mut self,
        action: &str,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        match action {
            "pause" => {
                self.service.send(Command::TogglePause);
            }
            "quick" => {
                hide(window, cx);
                open_window(Kind::Quick, cx);
            }
            "manager" => {
                hide(window, cx);
                open_window(Kind::Main, cx);
            }
            "settings" => {
                hide(window, cx);
                open_window(Kind::Settings, cx);
            }
            "quit" => {
                hide(window, cx);
                self.service.send(Command::Shutdown);
            }
            _ => {}
        }
    }
}
