use gpui_kit::{
    base::h_flex,
    component::{
        button::{Button, ButtonVariants},
        sidebar::{Sidebar, SidebarMenu, SidebarMenuItem},
        Sizable,
    },
    div,
    prelude::FluentBuilder,
    Context, IntoElement, ParentElement, Styled,
};
use gpui_kit_assets::IconName;

use crate::app::{Page, SoundboardApp};

impl SoundboardApp {
    pub fn render_sidebar(&mut self, cx: &mut Context<Self>) -> impl IntoElement {
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
}
