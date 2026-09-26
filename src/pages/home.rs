use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::{button::Button, h_flex, v_flex, ActiveTheme};
use gpui_kit::prelude::FluentBuilder;
use gpui_kit::{
    div, img, px, Context, InteractiveElement, IntoElement, ParentElement as _,
    StatefulInteractiveElement, Styled as _,
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
        let play_id = sound.id;
        let select_id = sound.id;
        let delete_id = sound.id;
        let has_keybind = sound.keybind.is_some();

        h_flex()
            .id(("sound-card", sound.id))
            .w(px(220.))
            .p_3()
            .gap_2()
            .items_center()
            .rounded(cx.theme().radius)
            .border_1()
            .border_color(cx.theme().border)
            .bg(cx.theme().muted)
            .child(match &sound.image_path {
                Some(path) => div()
                    .w(px(32.))
                    .h(px(32.))
                    .rounded_full()
                    .overflow_hidden()
                    .flex_shrink_0()
                    .child(img(path.clone()).w(px(32.)).h(px(32.)))
                    .into_any_element(),
                None => div()
                    .w(px(32.))
                    .h(px(32.))
                    .rounded_full()
                    .flex_shrink_0()
                    .bg(cx.theme().border)
                    .into_any_element(),
            })
            .child(
                Button::new(("play-sound", play_id))
                    .ghost()
                    .icon(IconName::Play)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.play_sound(play_id, cx);
                    })),
            )
            .child(
                div()
                    .id(("sound-name", select_id))
                    .flex_1()
                    .cursor_pointer()
                    .child(sound.name.clone())
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.select_sound(select_id, cx);
                    })),
            )
            .when(has_keybind, |this| {
                this.child(
                    div()
                        .text_xs()
                        .text_color(cx.theme().muted_foreground)
                        .child(sound.keybind.clone().unwrap_or_default()),
                )
            })
            .child(
                Button::new(("delete-sound", delete_id))
                    .ghost()
                    .icon(IconName::TrashOff)
                    .on_click(cx.listener(move |this, _, _, cx| {
                        this.remove_sound(delete_id, cx);
                    })),
            )
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
}
