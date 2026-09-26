use gpui_kit::{
    component::{
        button::{Button, ButtonVariants as _},
        h_flex,
        sidebar::{Sidebar, SidebarMenu, SidebarMenuItem},
        slider::{Slider, SliderState},
        switch::Switch,
        v_flex, ActiveTheme, IconName, Root, Side, Sizable as _,
    },
    prelude::FluentBuilder,
    InteractiveElement,
};
use gpui_kit::{
    div, px, App, AppContext as _, Context, Entity, IntoElement, ParentElement as _,
    PathPromptOptions, Render, Styled as _, Window,
};

use crate::audio::AudioEngine;
use crate::sound::{SoundEntry, SoundLibrary};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Page {
    Home,
    Settings,
}

pub struct SoundboardApp {
    page: Page,
    sidebar_collapsed: bool,
    library: SoundLibrary,
    audio: AudioEngine,
    volume: Entity<SliderState>,
    stop_others: bool,
}

impl SoundboardApp {
    pub fn new(_window: &mut Window, cx: &mut Context<Self>) -> Self {
        let library = SoundLibrary::load();
        let audio = AudioEngine::new().expect("failed to open the default audio output");
        let volume = cx.new(|_| SliderState::new().min(0.).max(100.).default_value(80.));

        // Re-render whenever the volume slider changes so the shown value stays in sync.
        cx.subscribe(&volume, |_this, _state, _event, cx| {
            cx.notify();
        })
        .detach();

        Self {
            page: Page::Home,
            sidebar_collapsed: false,
            library,
            audio,
            volume,
            stop_others: false,
        }
    }

    fn volume_fraction(&self, cx: &App) -> f32 {
        self.volume.read(cx).value().start() / 100.0
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
        self.sidebar_collapsed = !self.sidebar_collapsed;
        cx.notify();
    }

    fn add_sound(&mut self, cx: &mut Context<Self>) {
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

    fn play_sound(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(entry) = self.library.sounds.iter().find(|s| s.id == id).cloned() else {
            return;
        };
        if self.stop_others {
            self.audio.stop_all();
        }
        let volume = self.volume_fraction(cx);
        if let Err(err) = self.audio.play(&entry.path, volume) {
            eprintln!("soundboard: failed to play {:?}: {err:?}", entry.path);
        }
    }

    fn remove_sound(&mut self, id: u64, cx: &mut Context<Self>) {
        self.library.remove(id);
        let _ = self.library.save();
        cx.notify();
    }

    fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let collapsed = self.sidebar_collapsed;

        Sidebar::new(0)
            .collapsed(collapsed)
            .header(
                h_flex()
                    .items_center()
                    .justify_between()
                    .gap_2()
                    .px_2()
                    .py_1()
                    .when(!collapsed, |this| {
                        this.child(div().text_lg().child("Soundboard"))
                    })
                    .child(
                        Button::new("toggle-sidebar")
                            .ghost()
                            .small()
                            .icon(IconName::PanelLeft)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.toggle_sidebar(cx);
                            })),
                    ),
            )
            .child(
                SidebarMenu::new()
                    .child(
                        SidebarMenuItem::new("Home")
                            .icon(IconName::Battery)
                            .active(self.page == Page::Home)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = Page::Home;
                                cx.notify();
                            })),
                    )
                    .child(
                        SidebarMenuItem::new("Settings")
                            .icon(IconName::Settings)
                            .active(self.page == Page::Settings)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.page = Page::Settings;
                                cx.notify();
                            })),
                    ),
            )
    }

    fn render_sound_card(&self, sound: &SoundEntry, cx: &mut Context<Self>) -> impl IntoElement {
        let play_id = sound.id;
        let delete_id = sound.id;

        h_flex()
            .id(("sound-card", sound.id))
            .w(px(200.))
            .p_3()
            .gap_2()
            .items_center()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .child(
                Button::new(("play-sound", play_id))
                    .ghost()
                    .icon(IconName::Play)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.play_sound(play_id, cx);
                    })),
            )
            .child(div().flex_1().child(sound.name.clone()))
            .child(
                Button::new(("delete-sound", delete_id))
                    .ghost()
                    .icon(IconName::BatteryWarning)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.remove_sound(delete_id, cx);
                    })),
            )
    }

    fn render_home(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let sounds = self.library.sounds.clone();
        let has_sounds = !sounds.is_empty();

        v_flex()
            .size_full()
            .p_6()
            .gap_4()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_lg().child("My Sounds"))
                    .child(
                        Button::new("add-sound")
                            .primary()
                            .icon(IconName::Plus)
                            .label("Add sound…")
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.add_sound(cx);
                            })),
                    ),
            )
            .child(if has_sounds {
                div()
                    .flex()
                    .flex_wrap()
                    .gap_3()
                    .children(sounds.iter().map(|sound| self.render_sound_card(sound, cx)))
                    .into_any_element()
            } else {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(cx.theme().muted_foreground)
                    .child("No sounds yet — click \"Add sound…\" to pick a local audio file.")
                    .into_any_element()
            })
    }

    fn render_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let volume_percent = (self.volume_fraction(cx) * 100.0).round() as i32;

        v_flex()
            .size_full()
            .p_6()
            .gap_6()
            .child(div().text_lg().child("Settings"))
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .child(format!("Playback volume — {volume_percent}%")),
                    )
                    .child(Slider::new(&self.volume)),
            )
            .child(
                Switch::new("stop-others")
                    .label("Stop other sounds when playing a new one")
                    .checked(self.stop_others)
                    .on_change(cx.listener(|this, checked: &bool, _, cx| {
                        this.stop_others = *checked;
                        cx.notify();
                    })),
            )
            .child(
                Button::new("clear-sounds")
                    .danger()
                    .outline()
                    .label("Remove all sounds")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.library.sounds.clear();
                        let _ = this.library.save();
                        cx.notify();
                    })),
            )
    }
}

impl Render for SoundboardApp {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let content = match self.page {
            Page::Home => self.render_home(cx).into_any_element(),
            Page::Settings => self.render_settings(cx).into_any_element(),
        };

        h_flex()
            .id("soundboard-root")
            .items_stretch()
            .size_full()
            .bg(cx.theme().background)
            .text_color(cx.theme().foreground)
            .child(self.render_sidebar(cx))
            .child(div().flex_1().min_w_0().h_full().child(content))
            .children(Root::render_dialog_layer(window, cx))
            .children(Root::render_sheet_layer(window, cx))
            .children(Root::render_notification_layer(window, cx))
    }
}
