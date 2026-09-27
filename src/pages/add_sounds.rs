use gpui_kit::base::Disableable;
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::input::Input;
use gpui_kit::component::scroll::ScrollableElement;
use gpui_kit::component::Sizable;
use gpui_kit::component::{button::Button, h_flex, v_flex, ActiveTheme, Icon};
use gpui_kit::{div, px, Context, InteractiveElement, IntoElement, ParentElement, Styled as _};
use gpui_kit_assets::IconName;

use crate::core::get_sounds::InstantSound;
use crate::SoundboardApp;

impl SoundboardApp {
    pub fn render_online_result_card(
        &self,
        sound: &InstantSound,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let play_sound = sound.clone();
        let download_sound = sound.clone();

        v_flex()
            .id("online-sound-card")
            .w(px(160.))
            .p_3()
            .gap_2()
            .items_center()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().background)
            .shadow_sm()
            .hover(|style| {
                style
                    .bg(cx.theme().secondary)
                    .border_color(cx.theme().primary)
            })
            .child(
                div()
                    .w(px(130.))
                    .h(px(130.))
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().muted)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(IconName::Music),
                    ),
            )
            .child(
                div()
                    .text_xs()
                    .text_center()
                    .w_full()
                    .line_clamp(2)
                    .overflow_hidden()
                    .child(sound.title.clone()),
            )
            .child(
                div()
                    .gap_2()
                    .flex()
                    .child(
                        Button::new("play-btn")
                            .primary()
                            .small()
                            .icon(IconName::Play)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.play_online_sound(play_sound.clone(), cx);
                            })),
                    )
                    .child(
                        Button::new("download-btn")
                            .primary()
                            .small()
                            .icon(IconName::Plus)
                            .on_click(cx.listener(move |this, _, _, cx| {
                                this.download_and_add_sound(download_sound.clone(), cx);
                            })),
                    ),
            )
    }

    pub fn render_online_search(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let results = self.search_results.clone();
        let has_results = !results.is_empty();
        let result_count = results.len();
        let query_empty = self.search_query.trim().is_empty();

        v_flex()
            .size_full()
            .p_6()
            .gap_4()
            .child(
                v_flex()
                    .gap_1()
                    .child(div().text_lg().child("Search Myinstants Online"))
                    .child(if has_results {
                        div()
                            .text_xs()
                            .text_color(cx.theme().muted_foreground)
                            .child(format!("{} results", result_count))
                            .into_any_element()
                    } else {
                        div().into_any_element()
                    }),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(
                        div().flex_1().child(
                            Input::new(&self.search_input)
                                .prefix(Icon::new(IconName::Search).small())
                                .cleanable(true),
                        ),
                    )
                    .child(
                        Button::new("trigger-search")
                            .primary()
                            .label(if self.is_searching {
                                "Searching..."
                            } else {
                                "Search"
                            })
                            .disabled(self.is_searching || query_empty)
                            .on_click(cx.listener(|this, _, _, cx| {
                                this.search_online_sounds(cx);
                            })),
                    ),
            )
            .child(if self.is_searching {
                div()
                    .flex_1()
                    .flex()
                    .items_center()
                    .justify_center()
                    .text_color(cx.theme().muted_foreground)
                    .child("Searching online sounds...")
                    .into_any_element()
            } else if has_results {
                div()
                    .flex()
                    .overflow_y_scrollbar()
                    .flex_wrap()
                    .gap_3()
                    .children(
                        results
                            .iter()
                            .map(|sound| self.render_online_result_card(sound, cx)),
                    )
                    .into_any_element()
            } else {
                v_flex()
                    .flex_1()
                    .items_center()
                    .justify_center()
                    .gap_2()
                    .text_color(cx.theme().muted_foreground)
                    .child(div().child(IconName::Music))
                    .child("No online results found — try searching `Fahhh` .")
                    .into_any_element()
            })
    }
}
