use gpui_kit::{
    base::{h_flex, v_flex, Disableable, StyledExt},
    component::{
        button::{Button, ButtonVariants},
        ActiveTheme, Sizable,
    },
    div, img, px, Context, InteractiveElement, IntoElement, ObjectFit, ParentElement, Styled,
    StyledImage,
};
use gpui_kit_assets::IconName;

use crate::app::SoundboardApp;

impl SoundboardApp {
    pub fn render_detail_panel(&mut self, id: String, cx: &mut Context<Self>) -> impl IntoElement {
        let base_panel = v_flex()
            .id("sound-detail-panel")
            .w(px(320.))
            .h_full()
            .flex_shrink_0()
            .border_l_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .p_5()
            .gap_5();

        // check local sounds first then online
        if let Some(sound) = self.library.get(id.clone()).cloned() {
            let volume_percent = self
                .detail_volume
                .as_ref()
                .map(|slider| slider.read(cx).value().start().round() as i32)
                .unwrap_or((sound.volume * 100.0).round() as i32);

            let keybind_label = sound
                .keybind
                .clone()
                .unwrap_or_else(|| "No keybind set".to_string());

            return base_panel
                .child(
                    h_flex()
                        .items_center()
                        .justify_between()
                        .child(div().text_lg().child("Sound settings"))
                        .child(
                            Button::new("close-detail")
                                .ghost()
                                .icon(IconName::PanelRight)
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
                                .w_full()
                                .h(px(200.))
                                .overflow_hidden()
                                .child(
                                    img(path.clone())
                                        .w_full()
                                        .h_full()
                                        .object_fit(ObjectFit::Cover)
                                        .rounded(cx.theme().radius_lg),
                                )
                                .into_any_element(),
                            None => div()
                                .w_full()
                                .h(px(200.))
                                .child(IconName::Music)
                                .flex()
                                .items_center()
                                .justify_center()
                                .rounded(cx.theme().radius_lg)
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
                                .on_click(cx.listener({
                                    let id = id.clone();
                                    move |this, _, _, cx| {
                                        let val = id.clone();
                                        this.choose_image(&val, cx);
                                    }
                                })),
                        ),
                )
                .child(
                    h_flex().gap_1().flex().child(
                        Button::new("play-btn")
                            .primary()
                            .small()
                            .w_full()
                            .icon(IconName::Play)
                            .on_click(cx.listener({
                                let id = id.clone();
                                move |this, _, _, cx| {
                                    this.play_sound(id.clone(), cx);
                                }
                            })),
                    ),
                )
                .child(
                    v_flex()
                        .gap_2()
                        .child(div().text_sm().child(format!("Volume: {volume_percent}%")))
                        .child(
                            self.detail_volume
                                .as_ref()
                                .map(|slider| {
                                    gpui_kit::component::slider::Slider::new(slider)
                                        .into_any_element()
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
                                                .large()
                                                .tooltip("record keybind")
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
                                                .disabled(sound.keybind.is_none())
                                                .secondary()
                                                .large()
                                                .danger()
                                                .icon(IconName::KeyboardOff)
                                                .on_click(cx.listener({
                                                    let id = id.clone();
                                                    move |this, _, _, cx| {
                                                        this.clear_keybind(id.clone(), cx);
                                                    }
                                                })),
                                        ),
                                ),
                        )
                        .child(
                            Button::new("remove-sound")
                                .danger()
                                .w_full()
                                .large()
                                .disabled(self.is_busy)
                                .label("Delete sound")
                                .tooltip("remove sound")
                                .icon(IconName::Trash)
                                .on_click(cx.listener(move |this, _, _, cx| {
                                    if this.is_busy {
                                        return;
                                    }
                                    this.is_busy = true;
                                    cx.notify();
                                    this.audio.stop_all();
                                    this.remove_sound(id.clone(), cx);

                                    this.is_busy = false;
                                    cx.notify();
                                })),
                        ),
                )
                .into_any_element();
        }

        // panel for online sound details (will merge later)
        if let Ok(index) = id.parse::<usize>() {
            if let Some(online_sound) = self.search_results.get(index).cloned() {
                let download_sound = online_sound.clone();
                let play_sound = online_sound.clone();

                return base_panel
                    .child(
                        h_flex()
                            .items_center()
                            .justify_between()
                            .child(div().text_lg().child("Online Sound Preview"))
                            .child(
                                Button::new("close-detail")
                                    .ghost()
                                    .icon(IconName::PanelRight)
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.close_detail(cx);
                                    })),
                            ),
                    )
                    .child(
                        v_flex()
                            .flex_1()
                            .gap_4()
                            .child(
                                div()
                                    .w_full()
                                    .h(px(200.))
                                    .child(IconName::Music)
                                    .flex()
                                    .items_center()
                                    .justify_center()
                                    .rounded(cx.theme().radius_lg)
                                    .bg(cx.theme().muted),
                            )
                            .child(
                                div()
                                    .text_sm()
                                    .font_bold()
                                    .child(online_sound.title.clone()),
                            )
                            .child(
                                div()
                                    .text_xs()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Source: Myinstants Online"),
                            )
                            .child(
                                h_flex()
                                    .gap_2()
                                    .w_full()
                                    .child(
                                        Button::new("preview-play-btn")
                                            .primary()
                                            .flex_1()
                                            .icon(IconName::Play)
                                            .label("Play")
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                this.play_online_sound(play_sound.clone(), cx);
                                            })),
                                    )
                                    .child(
                                        Button::new("preview-download-btn")
                                            .outline()
                                            .flex_1()
                                            .disabled(self.is_busy)
                                            .icon(IconName::Plus)
                                            .label("Add to Library")
                                            .on_click(cx.listener(move |this, _, _, cx| {
                                                if this.is_busy {
                                                    return;
                                                }
                                                this.is_busy = true;
                                                cx.notify();
                                                this.download_and_add_sound(
                                                    download_sound.clone(),
                                                    cx,
                                                );
                                                this.is_busy = false;
                                                cx.notify();
                                            })),
                                    ),
                            ),
                    )
                    .into_any_element();
            }
        }

        base_panel
            .child(div().child("Sound not found"))
            .into_any_element()
    }

    pub fn close_detail(&mut self, cx: &mut Context<Self>) {
        self.selected_sound = None;
        self.detail_volume = None;
        self.details_page_expanded = false;
        self.recording_keybind = false;
        cx.notify();
    }

    fn clear_keybind(&mut self, id: String, cx: &mut Context<Self>) {
        if let Some(sound) = self.library.get_mut(id) {
            sound.keybind = None;
            let _ = self.library.save();
        }
        cx.notify();
    }

    pub fn start_recording_keybind(&mut self, cx: &mut Context<Self>) {
        self.recording_keybind = true;
        cx.notify();
    }

    pub fn remove_sound(&mut self, id: String, cx: &mut Context<Self>) {
        self.library.remove(id.clone());
        let _ = self.library.save();
        if self.selected_sound == Some(id) {
            self.selected_sound = None;
            self.detail_volume = None;
        }
        cx.notify();
    }
}
