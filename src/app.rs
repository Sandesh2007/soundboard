use gpui_kit::accesskit::Uuid;
use gpui_kit::base::input::{InputEvent, InputState};
use gpui_kit::component::notification::{Notification, NotificationType};
use gpui_kit::component::WindowExt;
use gpui_kit::{
    base::slider::SliderState,
    component::{h_flex, ActiveTheme, Root, ThemeRegistry},
    SharedString,
};
use gpui_kit::{
    div, Anchor, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, PathPromptOptions, Render, Styled as _, Window,
};

use crate::sound::SoundLibrary;
use crate::{audio::AudioEngine, core::get_sounds::InstantSound};

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Settings,
    AddSounds,
}

pub struct SoundboardApp {
    pub page: Page,
    pub sidebar_collapsed: bool,
    pub library: SoundLibrary,
    pub audio: AudioEngine,
    pub master_volume: Entity<gpui_kit::component::slider::SliderState>,
    pub stop_others: bool,

    // Right-side per-sound detail panel.
    pub selected_sound: Option<String>,
    pub detail_volume: Option<Entity<gpui_kit::component::slider::SliderState>>,
    pub recording_keybind: bool,
    pub details_page_expanded: bool,
    pub is_busy: bool,
    pub keybind_conflict_warning: String,

    pub focus_handle: FocusHandle,

    // search sounds
    pub search_query: String,
    pub search_input: Entity<InputState>,
    pub search_results: Vec<InstantSound>,
    pub is_searching: bool,
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

        let search_input =
            cx.new(|cx| InputState::new(window, cx).placeholder("Search Myinstants..."));

        cx.subscribe_in(
            &search_input,
            window,
            |this, state, event, _window, cx| match event {
                InputEvent::Change => {
                    this.search_query = state.read(cx).value().to_string();
                    cx.notify();
                }
                InputEvent::PressEnter { .. } => {
                    this.search_online_sounds(cx);
                }
                _ => {}
            },
        )
        .detach();

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
            is_busy: false,
            keybind_conflict_warning: String::new(),
            recording_keybind: false,
            focus_handle,
            search_query: String::new(),
            search_input,
            search_results: Vec::new(),
            is_searching: false,
        }
    }

    pub fn assign_keybind_to_sound(
        &mut self,
        target_id: &str,
        new_keybind: String,
        cx: &mut Context<Self>,
    ) {
        let conflict =
            self.library.sounds.iter().any(|sound| {
                sound.id != target_id && sound.keybind.as_deref() == Some(&new_keybind)
            });

        if conflict {
            self.keybind_conflict_warning = format!(
                "Keybind '{}' is already assigned to another sound!",
                new_keybind
            );
            self.show_toast(
                self.keybind_conflict_warning.clone(),
                NotificationType::Error,
                Anchor::BottomRight,
                cx,
            );
            cx.notify();
            return;
        }

        self.keybind_conflict_warning = String::new();
        if let Some(sound) = self.library.get_mut(target_id.to_string()) {
            sound.keybind = Some(new_keybind);
            let _ = self.library.save();
        }

        self.recording_keybind = false;
        cx.notify();
    }

    pub fn show_toast(
        &self,
        message: impl Into<SharedString>,
        notif_type: NotificationType,
        placement: Anchor,
        cx: &mut Context<Self>,
    ) {
        let msg = message.into();

        // grab the first window then show notification there
        if let Some(window_handle) = cx.windows().first() {
            let _ = window_handle.update(cx, |_, window, cx| {
                window.push_notification(
                    Notification::new()
                        .message(msg)
                        .with_type(notif_type)
                        .placement(placement)
                        .autohide(true),
                    cx,
                );
            });
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
                        let id = format!(
                            "{}-{}",
                            name.to_lowercase().replace(' ', "-"),
                            Uuid::new_v4()
                        );
                        this.library.add(id, name, path);
                    }
                    let _ = this.library.save();
                    cx.notify();
                });
            }
        })
        .detach();
    }

    pub fn play_sound(&mut self, id: String, cx: &mut Context<Self>) {
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

    pub fn select_sound(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(entry) = self.library.get(id.clone()) {
            let initial_percent = entry.volume * 100.0;
            let slider = cx.new(|_| {
                SliderState::new()
                    .min(0.)
                    .max(150.)
                    .default_value(initial_percent)
            });
            let s_clone = id.clone();
            cx.subscribe(&slider, move |this, state, _event, cx| {
                let percent = state.read(cx).value().start();
                if let Some(sound) = this.library.get_mut(s_clone.clone()) {
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
            return;
        }

        if let Ok(index) = id.parse::<usize>() {
            if self.search_results.get(index).is_some() {
                self.selected_sound = Some(id);
                self.details_page_expanded = true;
                self.detail_volume = None;
                self.recording_keybind = false;
                cx.notify();
            }
        }
    }

    pub fn choose_image(&mut self, id: &String, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: false,
            prompt: Some("Select an image".into()),
        });
        let id = id.clone();
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
            if let Some(id) = self.selected_sound.clone() {
                if event.keystroke.key == "escape" {
                    self.recording_keybind = false;
                    cx.notify();
                    return;
                }
                self.assign_keybind_to_sound(&id, combo, cx);
            } else {
                self.recording_keybind = false;
                cx.notify();
            }
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
            Page::AddSounds => self.render_online_search(cx).into_any_element(),
        };
        let detail_panel = self
            .selected_sound
            .clone()
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
