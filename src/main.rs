use iced::Theme;
use lucide_icons::LUCIDE_FONT_BYTES;

use crate::app::State;

mod app;
mod audio;
mod model;
mod ui;

pub const APP_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> iced::Result {
    let settings = iced::Settings {
        fonts: vec![LUCIDE_FONT_BYTES.into()],
        ..Default::default()
    };

    iced::application(State::default, State::update, State::view)
        .title("Counter Example")
        .theme(Theme::TokyoNight)
        .settings(settings)
        .run()
}
