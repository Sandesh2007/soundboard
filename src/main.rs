mod app;
mod assets;
mod audio;
mod core;
mod pages;
mod sound;
mod ui;

use std::path::PathBuf;

use gpui_kit::component::{Root, Theme, ThemeMode, ThemeRegistry};
use gpui_kit::*;

use crate::assets::Assets;
use app::SoundboardApp;

pub const APP_NAME: &str = env!("CARGO_BIN_NAME");

#[tokio::main]
async fn main() {
    let application = gpui_kit::application().with_assets(Assets);

    application.run(move |cx| {
        gpui_kit::init(cx);

        let themes_path = PathBuf::from("./themes");

        if let Err(err) = ThemeRegistry::watch_dir(themes_path, cx, |_cx| {
            // This closure runs automatically whenever theme files are loaded or updated
            println!("Themes directory loaded successfully.");
        }) {
            println!("Warning: Failed to load themes directory: {}", err);
        }

        Theme::change(ThemeMode::Dark, None, cx);

        cx.spawn(async move |cx| {
            cx.open_window(
                WindowOptions {
                    app_id: Some(APP_NAME.to_string()),
                    window_bounds: Some(WindowBounds::Windowed(Bounds {
                        origin: point(px(0.0), px(0.0)),
                        size: size(px(1200.0), px(760.0)),
                    })),
                    window_min_size: Some(size(px(1100.0), px(700.0))),
                    ..Default::default()
                },
                |window, cx| {
                    window.set_window_title("Soundboard");
                    let view = cx.new(|cx| SoundboardApp::new(window, cx));
                    cx.new(|cx| Root::new(view, window, cx))
                },
            )
            .expect("failed to open window");
        })
        .detach();
    });
}
