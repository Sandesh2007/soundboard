use gpui_kit::base::h_flex;
use gpui_kit::component::button::ButtonVariants;
use gpui_kit::component::{button::Button, slider::Slider, switch::Switch, v_flex};
use gpui_kit::component::{Theme, ThemeRegistry};
use gpui_kit::{div, Context, IntoElement, ParentElement as _, SharedString, Styled as _};

use crate::SoundboardApp;

impl SoundboardApp {
    pub fn render_settings(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
        let volume_percent = (self.master_volume_fraction(cx) * 100.0).round() as i32;

        let mut available_themes: Vec<SharedString> =
            ThemeRegistry::global(cx).themes().keys().cloned().collect();
        available_themes.sort();

        v_flex()
            .size_full()
            .p_6()
            .gap_6()
            .child(div().text_lg().child("Settings"))
            .child(
                h_flex()
                    .gap_2()
                    .flex_wrap()
                    .children(available_themes.into_iter().map(|name| {
                        let button_id =
                            format!("theme-btn-{}", name.to_lowercase().replace(" ", "-"));
                        let theme_name = name.clone();

                        Button::new(button_id)
                            .outline()
                            .label(name)
                            .on_click(cx.listener(move |_this, _, _window, cx| {
                                if let Some(theme_config) =
                                    ThemeRegistry::global(cx).themes().get(&theme_name).cloned()
                                {
                                    Theme::global_mut(cx).apply_config(&theme_config);

                                    cx.refresh_windows();
                                }
                            }))
                    })),
            )
            .child(
                v_flex()
                    .gap_2()
                    .child(
                        div()
                            .text_sm()
                            .child(format!("Master volume — {volume_percent}%")),
                    )
                    .child(Slider::new(&self.master_volume)),
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
                    .label("Remove all sounds")
                    .on_click(cx.listener(|this, _, _, cx| {
                        this.library.sounds.clear();
                        this.selected_sound = None;
                        this.detail_volume = None;
                        let _ = this.library.save();
                        cx.notify();
                    })),
            )
    }
}
