mod app;
mod audio;
mod pages;
mod sound;

use std::path::PathBuf;

use gpui_kit::component::{Root, Theme, ThemeMode, ThemeRegistry};
use gpui_kit::*;

use app::SoundboardApp;

fn main() {
    let application = gpui_kit::application().with_assets(gpui_kit::assets::Assets);

    application.run(move |cx| {
        gpui_kit::init(cx);

        let themes_path = PathBuf::from("./themes");

        if let Err(err) = ThemeRegistry::watch_dir(themes_path, cx, |_cx| {
            // This closure runs automatically whenever theme files are loaded or updated
            println!("Themes directory loaded successfully.");
        }) {
            println!("Warning: Failed to load themes directory: {}", err);
        }

        let default_theme = SharedString::from("Ayu Dark");
        if let Some(theme_config) = ThemeRegistry::global(cx)
            .themes()
            .get(&default_theme)
            .cloned()
        {
            Theme::global_mut(cx).apply_config(&theme_config);
        } else {
            println!(
                "Warning: Initial theme '{}' not found in registry.",
                default_theme
            );
        }

        Theme::change(ThemeMode::Dark, None, cx);

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|cx| SoundboardApp::new(window, cx));
                cx.new(|cx| Root::new(view, window, cx))
            })
            .expect("failed to open window");
        })
        .detach();
    });
}
