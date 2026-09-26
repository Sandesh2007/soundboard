mod app;
mod audio;
mod sound;

use gpui_kit::component::Root;
use gpui_kit::*;

use app::SoundboardApp;

fn main() {
    let application = gpui_kit::application().with_assets(gpui_kit::assets::Assets);

    application.run(move |cx| {
        gpui_kit::init(cx);

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
