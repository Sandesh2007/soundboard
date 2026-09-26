use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::kbd::Kbd;
use gpui_kit::component::{button::Button, h_flex, v_flex, ActiveTheme};
use gpui_kit::{
    div, img, px, Context, InteractiveElement, IntoElement, Keystroke, ObjectFit,
    ParentElement as _, StatefulInteractiveElement, Styled as _, StyledImage,
};
use gpui_kit_assets::IconName;

use crate::sound::SoundEntry;
use crate::SoundboardApp;

impl SoundboardApp {
    pub fn render_sound_card(
        &self,
        sound: &SoundEntry,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let select_id = sound.id;

        let keybind_element = match &sound.keybind {
            Some(kb) => match Keystroke::parse(kb) {
                Ok(keystroke) => Kbd::new(keystroke).into_any_element(),
                Err(_) => div()
                    .px_0p5()
                    .text_xs()
                    .rounded(cx.theme().radius)
                    .bg(cx.theme().danger)
                    .border_color(cx.theme().border)
                    .text_color(cx.theme().muted_foreground)
                    .child("Invalid keybind")
                    .into_any_element(),
            },
            None => div()
                .px_0p5()
                .text_xs()
                .rounded_sm()
                .bg(cx.theme().secondary)
                .border_color(cx.theme().border)
                .text_xs()
                .text_color(cx.theme().muted_foreground)
                .child("No keybind set")
                .into_any_element(),
        };

        v_flex()
            .id(("sound-card", sound.id))
            .w(px(170.))
            .p_2()
            .gap_2()
            .items_center()
            .rounded(cx.theme().radius_lg)
            .border_1()
            .border_color(cx.theme().border)
            .cursor_pointer()
            .relative()
            .on_click(cx.listener(move |this, _, _, cx| {
                this.select_sound(select_id, cx);
                this.play_sound(select_id, cx);
            }))
            .child(match &sound.image_path {
                Some(path) => div()
                    .w(px(140.))
                    .h(px(140.))
                    .rounded(cx.theme().radius_lg)
                    .overflow_hidden()
                    .flex_shrink_0()
                    .child(
                        img(path.clone())
                            .w_full()
                            .h_full()
                            .rounded(cx.theme().radius_lg)
                            .object_fit(ObjectFit::Fill),
                    )
                    .into_any_element(),
                None => div()
                    .w(px(140.))
                    .h(px(140.))
                    .rounded(cx.theme().radius_lg)
                    .flex_shrink_0()
                    .bg(cx.theme().border)
                    .flex()
                    .items_center()
                    .justify_center()
                    .child(
                        div()
                            .text_color(cx.theme().muted_foreground)
                            .child(IconName::Music),
                    )
                    .into_any_element(),
            })
            .child(
                div()
                    .text_xs()
                    .text_center()
                    .w_full()
                    .child(sound.name.clone()),
            )
            .child(keybind_element)
    }

    pub fn render_home(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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
                        h_flex()
                            .gap_2()
                            .child(
                                Button::new("add-sound")
                                    .primary()
                                    .icon(IconName::Plus)
                                    .label("Add sound…")
                                    .on_click(cx.listener(|this, _, _, cx| {
                                        this.add_sound(cx);
                                    })),
                            )
                            .child(if self.details_page_expanded {
                                div().invisible()
                            } else {
                                div()
                                    .child(
                                        Button::new("open-detail")
                                            .secondary()
                                            .icon(IconName::PanelLeft)
                                            .on_click(cx.listener(|this, _, _, cx| {
                                                this.select_sound(0, cx);
                                            })),
                                    )
                                    .visible()
                            }),
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
}
