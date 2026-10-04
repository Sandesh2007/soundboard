use crate::config::Config;
use crate::core::keybind::GlobalShortcutManager;
use crate::sound::SoundLibrary;
use crate::{audio::AudioEngine, core::get_sounds::InstantSound};
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
    IntoElement, ParentElement as _, PathPromptOptions, Render, Styled as _, Window,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};

pub const _BASE_URL: &str = "https://soundboard-api.vercel.app/";
pub const QUERY_URL: &str = "https://soundboard-api.vercel.app/search?q={}";
pub const _TRENDING_URL: &str = "https://soundboard-api.vercel.app/trending?q=id";

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Page {
    Home,
    Settings,
    AddSounds,
}

pub struct SoundboardApp {
    pub _config: Config,

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

    pub playing_id: Option<String>,
    pub playing_duration: Option<Duration>,
    pub playback_elapsed: Duration,
    pub playback_started_at: Option<Instant>,
    pub is_paused: bool,

    pub download_progress: Option<f32>,
    pub focus_handle: FocusHandle,

    // search sounds
    pub search_query: String,
    pub search_input: Entity<InputState>,
    pub search_results: Vec<InstantSound>,
    pub is_searching: bool,

    pub global_shortcuts: Option<GlobalShortcutManager>,
}

impl SoundboardApp {
    pub fn new(window: &mut Window, cx: &mut Context<Self>, config: Config) -> Self {
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
            _config: config,
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
            playing_id: None,
            playing_duration: None,
            playback_elapsed: Duration::ZERO,
            playback_started_at: None,
            is_paused: false,
            download_progress: None,
            focus_handle,
            search_query: String::new(),
            search_input,
            search_results: Vec::new(),
            is_searching: false,
            global_shortcuts: None,
        }
    }

    pub fn start_global_shortcuts(&mut self, cx: &mut Context<Self>) {
        let app = cx.entity().downgrade();

        let manager = GlobalShortcutManager::start(app, cx);

        self.global_shortcuts = Some(manager);
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

        if let Some(manager) = &self.global_shortcuts {
            manager.reload();
        }

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

    fn begin_playback(&mut self, id: String, path: PathBuf, volume: f32, cx: &mut Context<Self>) {
        if self.stop_others {
            self.audio.stop_all();
        }
        match self.audio.play(id.clone(), &path, volume) {
            Ok(duration) => {
                self.playing_id = Some(id.clone());
                self.playing_duration = duration;
                self.playback_elapsed = Duration::ZERO;
                self.playback_started_at = Some(Instant::now());
                self.is_paused = false;
                cx.notify();
                self.watch_playback(id, cx);
            }
            Err(err) => {
                eprintln!("soundboard: failed to play {:?}: {err:?}", path);
            }
        }
    }

    pub fn play_sound(&mut self, id: String, cx: &mut Context<Self>) {
        let Some(entry) = self.library.get(id.clone()).cloned() else {
            return;
        };
        let volume = self.master_volume_fraction(cx) * entry.volume;
        self.begin_playback(id, entry.path, volume, cx);
    }

    /// Used by `play_online_sound` once a preview file is cached on disk.
    pub fn play_cached_path(&mut self, id: String, path: PathBuf, cx: &mut Context<Self>) {
        let volume = self.master_volume_fraction(cx);
        self.begin_playback(id, path, volume, cx);
    }

    fn toggle_active(&mut self, id: &str, cx: &mut Context<Self>) -> bool {
        if self.playing_id.as_deref() != Some(id) || self.audio.has_finished(id) {
            return false;
        }
        if self.is_paused {
            self.audio.resume(id);
            self.playback_started_at = Some(Instant::now());
            self.is_paused = false;
            cx.notify();
            self.watch_playback(id.to_string(), cx);
        } else {
            self.audio.pause(id);
            if let Some(started) = self.playback_started_at.take() {
                self.playback_elapsed += started.elapsed();
            }
            self.is_paused = true;
            cx.notify();
        }
        true
    }

    pub fn toggle_play_pause(&mut self, id: String, cx: &mut Context<Self>) {
        if !self.toggle_active(&id, cx) {
            self.play_sound(id, cx);
        }
    }

    pub fn toggle_preview_play_pause(&mut self, sound: InstantSound, cx: &mut Context<Self>) {
        let preview_id = format!("preview-{}", sound.mp3);
        if !self.toggle_active(&preview_id, cx) {
            self.play_online_sound(sound, cx);
        }
    }

    /// Polls playback state every 100ms and calls cx.notify() so the
    /// progress bar advances, stopping itself once the sound finishes,
    /// is paused, or is superseded by a different sound.
    /// this looks like a heafty load ;;)
    fn watch_playback(&mut self, id: String, cx: &mut Context<Self>) {
        cx.spawn(async move |this, cx| loop {
            cx.background_executor()
                .timer(Duration::from_millis(100))
                .await;

            let should_stop = this
                .update(cx, |this, cx| {
                    if this.playing_id.as_deref() != Some(id.as_str()) || this.is_paused {
                        return true;
                    }
                    if this.audio.has_finished(&id) {
                        this.playing_id = None;
                        this.playback_started_at = None;
                        this.playback_elapsed = Duration::ZERO;
                        this.playing_duration = None;
                        cx.notify();
                        return true;
                    }
                    cx.notify();
                    false
                })
                .unwrap_or(true);

            if should_stop {
                break;
            }
        })
        .detach();
    }

    /// Fraction (0.0–1.0) through playback for `id`, or `None` if it isn't
    /// the active sound or has no known duration.
    pub fn playback_fraction_for(&self, id: &str) -> Option<f32> {
        if self.playing_id.as_deref() != Some(id) {
            return None;
        }
        let duration = self.playing_duration?;
        if duration.as_secs_f32() <= 0.0 {
            return None;
        }
        let elapsed = self.playback_elapsed
            + self
                .playback_started_at
                .map(|s| s.elapsed())
                .unwrap_or_default();
        Some((elapsed.as_secs_f32() / duration.as_secs_f32()).min(1.0))
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
