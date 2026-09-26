use gpui_kit::{
    base::v_flex,
    component::{
        button::{Button, ButtonVariants as _},
        h_flex,
        sidebar::{Sidebar, SidebarMenu, SidebarMenuItem},
        ActiveTheme, Root, Sizable as _, ThemeRegistry,
    },
    prelude::FluentBuilder,
    SharedString,
};
use gpui_kit::{
    div, img, px, App, AppContext as _, Context, Entity, FocusHandle, InteractiveElement as _,
    IntoElement, KeyDownEvent, ParentElement as _, PathPromptOptions, Render, Styled as _, Window,
};
use gpui_kit_assets::IconName;

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
            recording_keybind: false,
            focus_handle,
        }
    }

    pub fn master_volume_fraction(&self, cx: &App) -> f32 {
        self.master_volume.read(cx).value().start() / 100.0
    }

    fn toggle_sidebar(&mut self, cx: &mut Context<Self>) {
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

    pub fn remove_sound(&mut self, id: u64, cx: &mut Context<Self>) {
        self.library.remove(id);
        let _ = self.library.save();
        if self.selected_sound == Some(id) {
            self.selected_sound = None;
            self.detail_volume = None;
        }
        cx.notify();
    }

    pub fn select_sound(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(entry) = self.library.get(id) else {
            return;
        };
        let initial_percent = entry.volume * 100.0;
        let slider = cx.new(|_| {
            gpui_kit::component::slider::SliderState::new()
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
        self.detail_volume = Some(slider);
        self.recording_keybind = false;
        cx.notify();
    }

    fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.selected_sound = None;
        self.detail_volume = None;
        self.recording_keybind = false;
        cx.notify();
    }

    fn start_recording_keybind(&mut self, cx: &mut Context<Self>) {
        self.recording_keybind = true;
        cx.notify();
    }

    fn clear_keybind(&mut self, id: u64, cx: &mut Context<Self>) {
        if let Some(sound) = self.library.get_mut(id) {
            sound.keybind = None;
            let _ = self.library.save();
        }
        cx.notify();
    }

    fn choose_image(&mut self, id: u64, cx: &mut Context<Self>) {
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
                            .icon(IconName::House)
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

    fn render_detail_panel(&mut self, id: u64, cx: &mut Context<Self>) -> impl IntoElement {
        let Some(sound) = self.library.get(id).cloned() else {
            return div().into_any_element();
        };
        let volume_percent = self
            .detail_volume
            .as_ref()
            .map(|slider| slider.read(cx).value().start().round() as i32)
            .unwrap_or((sound.volume * 100.0).round() as i32);

        let keybind_label = sound
            .keybind
            .clone()
            .unwrap_or_else(|| "No keybind set".to_string());

        v_flex()
            .id("sound-detail-panel")
            .w(px(320.))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .p_5()
            .gap_5()
            .child(
                h_flex()
                    .items_center()
                    .justify_between()
                    .child(div().text_lg().child("Sound settings"))
                    .child(
                        Button::new("close-detail")
                            .ghost()
                            .icon(IconName::X)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.close_detail(cx);
                            })),
                    ),
            )
            .child(
                v_flex()
                    .items_center()
                    .gap_2()
                    .child(match &sound.image_path {
                        Some(path) => div()
                            .w(px(80.))
                            .h(px(80.))
                            .rounded_full()
                            .overflow_hidden()
                            .child(img(path.clone()).w(px(80.)).h(px(80.)))
                            .into_any_element(),
                        None => div()
                            .w(px(80.))
                            .h(px(80.))
                            .rounded_full()
                            .bg(cx.theme().muted)
                            .into_any_element(),
                    })
                    .child(div().text_sm().child(sound.name.clone()))
                    .child(
                        Button::new("change-image")
                            .outline()
                            .small()
                            .icon(IconName::Image)
                            .label("Change image…")
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.choose_image(id, cx);
                            })),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(div().text_sm().child(format!("Volume — {volume_percent}%")))
                    .child(
                        self.detail_volume
                            .as_ref()
                            .map(|slider| {
                                gpui_kit::component::slider::Slider::new(slider).into_any_element()
                            })
                            .unwrap_or_else(|| div().into_any_element()),
                    ),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(div().text_sm().child("Keybind"))
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .gap_2()
                            .child(div().text_color(cx.theme().muted_foreground).child(
                                if self.recording_keybind {
                                    "Press any key…".to_string()
                                } else {
                                    keybind_label
                                },
                            ))
                            .child(
                                h_flex()
                                    .gap_1()
                                    .child(
                                        Button::new("record-keybind")
                                            .outline()
                                            .small()
                                            .label(if self.recording_keybind {
                                                "Cancel"
                                            } else {
                                                "Record…"
                                            })
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if this.recording_keybind {
                                                    this.recording_keybind = false;
                                                    cx.notify();
                                                } else {
                                                    this.start_recording_keybind(cx);
                                                }
                                            })),
                                    )
                                    .child(
                                        Button::new("clear-keybind")
                                            .ghost()
                                            .small()
                                            .icon(IconName::TrashOff)
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.clear_keybind(id, cx);
                                            })),
                                    ),
                            ),
                    ),
            )
            .into_any_element()
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
