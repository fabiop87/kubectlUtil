#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]
use gpui_kit::*;
mod action;
mod app;
mod command;
mod theme;

fn main() {
    application().with_assets(assets::Assets).run(|cx| {
        init(cx);
        theme::install(cx);
        cx.on_window_closed(|cx, _| {
            if cx.windows().is_empty() {
                cx.quit();
            }
        })
        .detach();
        open_window(
            WindowOptions {
                window_bounds: Some(WindowBounds::Windowed(Bounds::centered(
                    None,
                    size(px(1280.), px(900.)),
                    cx,
                ))),
                window_min_size: Some(size(px(960.), px(720.))),
                titlebar: Some(TitlebarOptions {
                    title: Some("kubectlUtil".into()),
                    ..Default::default()
                }),
                ..Default::default()
            },
            cx,
            |window, cx| cx.new(|cx| app::AppView::new(window, cx)),
        )
        .expect("Não foi possível abrir o kubectlUtil");
        cx.activate(true);
    });
}
