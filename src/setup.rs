#![cfg_attr(windows, windows_subsystem = "windows")]
use gpui_kit::{
    component::{
        button::*,
        input::{Input, InputState},
        switch::Switch,
        *,
    },
    prelude::FluentBuilder,
    *,
};
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
    time::Duration,
};
use x_clipboard::installer::{self, Options};

struct Setup {
    path: Entity<InputState>,
    desktop: bool,
    start_menu: bool,
    launch: bool,
    busy: bool,
    done: bool,
    uninstall: bool,
    status: String,
    installed: Option<PathBuf>,
}
impl Setup {
    fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let uninstall = std::env::args().any(|a| a == "--uninstall");
        let target = if uninstall {
            std::env::current_exe()
                .unwrap()
                .parent()
                .unwrap()
                .to_path_buf()
        } else {
            PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default())
                .join("Programs/X Clipboard")
        };
        Self {
            path: cx.new(|cx| {
                InputState::new(window, cx).default_value(target.to_string_lossy().into_owned())
            }),
            desktop: true,
            start_menu: true,
            launch: true,
            busy: false,
            done: false,
            uninstall,
            status: String::new(),
            installed: None,
        }
    }
    fn browse(&mut self, window: &Window, cx: &mut Context<Self>) {
        let paths = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("选择 X Clipboard 安装目录".into()),
        });
        cx.spawn_in(window, async move |this, cx| {
            if let Ok(Ok(Some(paths))) = paths.await
                && let Some(path) = paths.first()
            {
                let value = path.to_string_lossy().into_owned();
                let _ = this.update_in(cx, |this, window, cx| {
                    this.path
                        .update(cx, |input, cx| input.set_value(value, window, cx));
                });
            }
        })
        .detach();
    }
    fn install(&mut self, cx: &mut Context<Self>) {
        let target = PathBuf::from(self.path.read(cx).value().trim());
        if self.uninstall {
            #[cfg(windows)]
            match installer::uninstall(&target) {
                Ok(()) => cx.quit(),
                Err(e) => {
                    self.status = format!("{e:#}");
                    cx.notify();
                }
            }
            return;
        }
        self.busy = true;
        self.status = "正在校验并安装应用…".into();
        let options = Options {
            target: target.clone(),
            desktop: self.desktop,
            start_menu: self.start_menu,
        };
        let result = Arc::new(Mutex::new(None));
        let worker = result.clone();
        std::thread::spawn(move || {
            let result = (|| {
                let exe = std::env::current_exe()?;
                let payload = installer::payload(&exe)?;
                installer::install(&payload, &exe, &options)
            })();
            *worker.lock().unwrap() = Some(result.map_err(|e: anyhow::Error| format!("{e:#}")));
        });
        cx.spawn(async move |this, cx| {
            loop {
                cx.background_executor()
                    .timer(Duration::from_millis(60))
                    .await;
                if let Some(result) = result.lock().unwrap().take() {
                    let _ = this.update(cx, |this, cx| {
                        this.busy = false;
                        match result {
                            Ok(warning) => {
                                this.done = true;
                                this.installed = Some(target);
                                this.status = warning
                                    .unwrap_or_else(|| "安装完成。下一次复制，从这里继续。".into());
                            }
                            Err(e) => this.status = e,
                        }
                        cx.notify();
                    });
                    break;
                }
            }
        })
        .detach();
        cx.notify();
    }
    fn toggle(
        &self,
        id: &'static str,
        label: &'static str,
        checked: bool,
        change: fn(&mut Self, bool),
        cx: &mut Context<Self>,
    ) -> AnyElement {
        div()
            .flex()
            .items_center()
            .justify_between()
            .py_2()
            .child(div().text_size(px(13.)).child(label))
            .child(
                Switch::new(id)
                    .small()
                    .accessibility_label(label)
                    .checked(checked)
                    .disabled(self.busy)
                    .on_click(cx.listener(move |this, on: &bool, _, cx| {
                        change(this, *on);
                        cx.notify();
                    })),
            )
            .into_any_element()
    }
}
impl Render for Setup {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let amber = rgb(0xffcc5c);
        let ink = rgb(0x242924);
        div()
            .size_full()
            .flex()
            .bg(rgb(0xfaf9f5))
            .text_color(ink)
            .child(
                div()
                    .w(px(260.))
                    .h_full()
                    .p_8()
                    .flex()
                    .flex_col()
                    .justify_between()
                    .bg(ink)
                    .text_color(rgb(0xf6f4e9))
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(13.))
                                    .text_color(amber)
                                    .child("X / CLIPBOARD"),
                            )
                            .child(
                                div()
                                    .mt_8()
                                    .text_size(px(36.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("让灵感"),
                            )
                            .child(
                                div()
                                    .text_size(px(36.))
                                    .font_weight(FontWeight::BOLD)
                                    .child("留在手边。"),
                            )
                            .child(div().mt_5().w(px(56.)).h(px(4.)).bg(amber))
                            .child(
                                div()
                                    .mt_5()
                                    .text_size(px(13.))
                                    .line_height(px(23.))
                                    .text_color(rgb(0xbec6ba))
                                    .child(
                                        "每一次复制，都有迹可循。\n本机存储 · 加密保护 · 快速回贴",
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .window_control_area(WindowControlArea::Drag)
                                    .text_size(px(11.))
                                    .text_color(amber)
                                    .child(format!("WINDOWS / {}", env!("CARGO_PKG_VERSION"))),
                            )
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgb(0xbec6ba))
                                    .child("A LITTLE SPACE FOR EVERYTHING."),
                            ),
                    ),
            )
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .p_8()
                    .flex()
                    .flex_col()
                    .gap_4()
                    .child(
                        div()
                            .flex()
                            .justify_between()
                            .items_center()
                            .child(div().text_size(px(11.)).text_color(rgb(0x7a8074)).child(
                                if self.done {
                                    "02 / READY"
                                } else {
                                    "01 / MAKE IT YOURS"
                                },
                            ))
                            .child(
                                Button::new("close")
                                    .ghost()
                                    .small()
                                    .label("关闭")
                                    .disabled(self.busy)
                                    .on_click(|_, _, cx| cx.quit()),
                            ),
                    )
                    .child(
                        div()
                            .mt_3()
                            .text_size(px(26.))
                            .font_weight(FontWeight::SEMIBOLD)
                            .child(if self.uninstall {
                                "卸载 X Clipboard"
                            } else if self.done && !self.launch {
                                "完成"
                            } else if self.done {
                                "一切就绪。"
                            } else {
                                "为你的桌面，留一份记忆。"
                            }),
                    )
                    .child(div().text_size(px(12.)).text_color(rgb(0x747c70)).child(
                        if self.uninstall {
                            "只移除应用文件，保留剪贴板历史和加密密钥。"
                        } else {
                            "选择一个位置，开始更从容的复制与粘贴。"
                        },
                    ))
                    .child(
                        div()
                            .mt_3()
                            .flex()
                            .flex_col()
                            .gap_2()
                            .child(
                                div()
                                    .text_size(px(12.))
                                    .font_weight(FontWeight::MEDIUM)
                                    .child("安装位置"),
                            )
                            .child(
                                div()
                                    .flex()
                                    .gap_2()
                                    .items_center()
                                    .child(
                                        div()
                                            .flex_1()
                                            .min_w_0()
                                            .child(Input::new(&self.path).disabled(
                                                self.busy || self.done || self.uninstall,
                                            )),
                                    )
                                    .when(!self.uninstall && !self.done, |d| {
                                        d.child(
                                            Button::new("browse")
                                                .label("浏览…")
                                                .disabled(self.busy)
                                                .on_click(cx.listener(|this, _, window, cx| {
                                                    this.browse(window, cx)
                                                })),
                                        )
                                    }),
                            ),
                    )
                    .when(!self.uninstall && !self.done, |d| {
                        d.child(
                            div()
                                .mt_2()
                                .flex()
                                .flex_col()
                                .child(self.toggle(
                                    "desktop",
                                    "创建桌面快捷方式",
                                    self.desktop,
                                    |s, v| s.desktop = v,
                                    cx,
                                ))
                                .child(self.toggle(
                                    "start-menu",
                                    "添加到开始菜单",
                                    self.start_menu,
                                    |s, v| s.start_menu = v,
                                    cx,
                                )),
                        )
                    })
                    .when(self.done, |d| {
                        d.child(self.toggle(
                            "launch",
                            "完成后启动 X Clipboard",
                            self.launch,
                            |s, v| s.launch = v,
                            cx,
                        ))
                    })
                    .child(
                        div()
                            .flex_1()
                            .min_h_0()
                            .text_size(px(12.))
                            .text_color(if self.done {
                                rgb(0x47765a)
                            } else {
                                rgb(0x8b6438)
                            })
                            .child(self.status.clone()),
                    )
                    .child(
                        div()
                            .flex()
                            .items_center()
                            .justify_between()
                            .child(
                                div()
                                    .text_size(px(10.))
                                    .text_color(rgb(0x747c70))
                                    .child("仅为当前用户安装 · 无需管理员权限"),
                            )
                            .child(
                                Button::new("install")
                                    .primary()
                                    .disabled(self.busy)
                                    .label(if self.busy {
                                        "正在安装…"
                                    } else if self.done {
                                        "开始使用"
                                    } else if self.uninstall {
                                        "确认卸载"
                                    } else {
                                        "安装 X Clipboard"
                                    })
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        if this.done {
                                            if this.launch
                                                && let Some(target) = &this.installed
                                            {
                                                #[cfg(windows)]
                                                {
                                                    use std::os::windows::process::CommandExt;
                                                    if let Err(e) = std::process::Command::new(
                                                        target.join("x-clipboard.exe"),
                                                    )
                                                    .current_dir(target)
                                                    .creation_flags(0x08000000)
                                                    .spawn()
                                                    {
                                                        this.status = format!("启动失败：{e}");
                                                        cx.notify();
                                                        return;
                                                    }
                                                }
                                            }
                                            cx.quit();
                                        } else {
                                            this.install(cx);
                                        }
                                    })),
                            ),
                    )
                    .child(
                        div()
                            .text_size(px(10.))
                            .text_color(rgb(0x94998d))
                            .child("使用即表示你了解本机剪贴板记录功能。开源组件许可随应用提供。"),
                    ),
            )
    }
}
fn main() {
    gpui_kit::application()
        .with_assets(gpui_kit::assets::AllAssets)
        .run(|cx| {
            gpui_kit::init(cx);
            Theme::change(ThemeMode::Light, None, cx);
            Theme::update(cx, |theme| {
                theme.font_family = "Microsoft YaHei UI".into();
                theme.button_primary = rgb(0xffcc5c).into();
                theme.button_primary_hover = rgb(0xf3bc41).into();
                theme.button_primary_active = rgb(0xe5ac32).into();
                theme.button_primary_foreground = rgb(0x242924).into();
                theme.primary = rgb(0xa57213).into();
                theme.ring = theme.primary;
                theme.switch = rgb(0xc4c9bd).into();
            });
            let opened = gpui_kit::open_window(
                WindowOptions {
                    window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                        None,
                        size(px(860.), px(550.)),
                        cx,
                    ))),
                    titlebar: None,
                    is_resizable: false,
                    ..Default::default()
                },
                cx,
                |window, cx| {
                    window.set_window_title("X Clipboard · 安装");
                    let view = cx.new(|cx| Setup::new(window, cx));
                    let guard = view.clone();
                    window.on_window_should_close(cx, move |_, cx| !guard.read(cx).busy);
                    view
                },
            );
            if let Ok((handle, _)) = opened {
                let _ = handle.update(cx, |_, window, _| {
                    #[cfg(windows)]
                    {
                        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
                        if let RawWindowHandle::Win32(handle) =
                            HasWindowHandle::window_handle(window).unwrap().as_raw()
                        {
                            x_clipboard::platform::show_window(handle.hwnd.get() as usize);
                        }
                    }
                    window.activate_window();
                });
            }
            cx.activate(true);
        });
}
