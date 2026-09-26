use gpui_kit::{
    base::slider::SliderState,
    component::{h_flex, ActiveTheme, Root, ThemeRegistry},
    SharedString,
};
use gpui_kit::{
    div, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _, IntoElement,
    KeyDownEvent, ParentElement as _, PathPromptOptions, Render, Styled as _, Window,
};

use crate::audio::AudioEngine;
use crate::sound::SoundLibrary;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Settings,
}

pub struct SoundboardApp {
    pub page: Page,
    pub sidebar_collapsed: bool,
    pub library: SoundLibrary,
    pub audio: AudioEngine,
    pub master_volume: Entity<gpui_kit::component::slider::SliderState>,
    pub stop_others: bool,

    // Right-side per-sound detail panel.
    pub selected_sound: Option<u64>,
    pub detail_volume: Option<Entity<gpui_kit::component::slider::SliderState>>,
    pub recording_keybind: bool,
    pub details_page_expanded: bool,

    pub focus_handle: FocusHandle,
}

impl SoundboardApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = SoundLibrary::load();
        let audio = AudioEngine::new().expect("failed to open the default audio output");
        let master_volume = cx.new(|_| {
            gpui_kit::component::slider::SliderState::new()
                .min(0.)
                .max(100.)
                .default_value(80.)
        });

        cx.subscribe(&master_volume, |_this, _state, _event, cx| {
            cx.notify();
        })
        .detach();

        let mut theme_names: Vec<SharedString> =
            ThemeRegistry::global(cx).themes().keys().cloned().collect();
        theme_names.sort();

        let focus_handle = cx.focus_handle();
        window.focus(&focus_handle, cx);

        Self {
            page: Page::Home,
            sidebar_collapsed: false,
            library,
            audio,
            master_volume,
            stop_others: true,
            selected_sound: None,
            detail_volume: None,
            details_page_expanded: false,
            recording_keybind: false,
            focus_handle,
        }
    }

    pub fn master_volume_fraction(&self, cx: &App) -> f32 {
        self.master_volume.read(cx).value().start() / 100.0
    }

    pub(crate) fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    pub fn add_sound(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: true,
            prompt: Some("Select sound files".into()),
        });

        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(paths))) = receiver.await {
                let _ = this.update(cx, |this, cx| {
                    for path in paths {
                        let name = path
                            .file_stem()
                            .map(|stem| stem.to_string_lossy().into_owned())
                            .unwrap_or_else(|| "Sound".to_string());
                        this.library.add(name, path);
                    }
                    let _ = this.library.save();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn play_sound(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(entry) = self.library.get(id).cloned() else {
            return;
        };
        if self.stop_others {
            self.audio.stop_all();
        }
        let volume = self.master_volume_fraction(cx) * entry.volume;
        if let Err(err) = self.audio.play(&entry.path, volume) {
            eprintln!("soundboard: failed to play {:?}: {err:?}", entry.path);
        }
    }

    pub fn select_sound(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(entry) = self.library.get(id) else {
            return;
        };
        let initial_percent = entry.volume * 100.0;
        let slider = cx.new(|_| {
            SliderState::new()
                .min(0.)
                .max(150.)
                .default_value(initial_percent)
        });
        cx.subscribe(&slider, move |this, state, _event, cx| {
            let percent = state.read(cx).value().start();
            if let Some(sound) = this.library.get_mut(id) {
                sound.volume = percent / 100.0;
                let _ = this.library.save();
            }
            cx.notify();
        })
        .detach();

        self.selected_sound = Some(id);
        self.details_page_expanded = true;
        self.detail_volume = Some(slider);
        self.recording_keybind = false;
        cx.notify();
    }

    pub fn choose_image(&mut self, id: u64, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select an image".into()),
        });
        cx.spawn(async move |this, cx| {
            if let Ok(Ok(Some(mut paths))) = receiver.await {
                if let Some(path) = paths.pop() {
                    let _ = this.update(cx, |this, cx| {
                        if let Some(sound) = this.library.get_mut(id) {
                            sound.image_path = Some(path);
                            let _ = this.library.save();
                        }
                        cx.notify();
                    });
                }
            }
        })
        .detach();
    }

    fn on_key_down(&mut self, event: &KeyDownEvent, _window: &mut Window, cx: &mut Context<Self>) {
        let combo = event.keystroke.to_string();

        if self.recording_keybind {
            if let Some(id) = self.selected_sound {
                if let Some(sound) = self.library.get_mut(id) {
                    sound.keybind = Some(combo);
                    let _ = self.library.save();
                }
            }
            self.recording_keybind = false;
            cx.notify();
            return;
        }

        if let Some(id) = self.library.find_by_keybind(&combo) {
            self.play_sound(id, cx);
        }
    }
}

impl Render for SoundboardApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page {
            Page::Home => self.render_home(cx).into_any_element(),
            Page::Settings => self.render_settings(cx).into_any_element(),
        };
        let detail_panel = self
            .selected_sound
            .map(|id| self.render_detail_panel(id, cx));

        h_flex()
            .id("soundboard-root")
            .track_focus(&self.focus_handle)
            .on_key_down(cx.listener(Self::on_key_down))
            .items_stretch()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_sidebar(cx))
            .child(div().flex_1().min_w_0().h_full().child(content))
            .children(detail_panel)
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
