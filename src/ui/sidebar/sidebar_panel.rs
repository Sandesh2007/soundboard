use crate::APP_VERSION;
use crate::app::{Message, State, Tab};
use iced::widget::{Space, button, column, container, row, rule, text};
use iced::{Element, Length, alignment};
use lucide_icons::iced::{
    icon_layout_dashboard, icon_panel_left_close, icon_panel_left_open, icon_settings,
};

pub fn view(state: &State) -> Element<Message> {
    let collapse_btn = button(if state.sidebar_collapsed {
        icon_panel_left_open()
    } else {
        icon_panel_left_close()
    })
    .on_press(Message::ToggleSidebar);

    let soundboard_btn = button(if state.sidebar_collapsed {
        row![icon_layout_dashboard()].width(Length::Shrink)
    } else {
        row![icon_layout_dashboard(), text("Soundboard")]
            .width(Length::Fill)
            .spacing(6)
    })
    .on_press(Message::SwitchTab(Tab::Soundboard));

    let settings_btn = button(if state.sidebar_collapsed {
        row![icon_settings()].width(Length::Shrink)
    } else {
        row![icon_settings(), text("Settings")]
            .width(Length::Fill)
            .spacing(6)
    })
    .on_press(Message::SwitchTab(Tab::Settings));

    let about_footer = column![text!("v{}", APP_VERSION).size(12)];
    let footer = container(about_footer).padding(4);

    let sidebar_width = if state.sidebar_collapsed {
        Length::Fixed(60.0)
    } else {
        Length::Fixed(220.0)
    };

    let content = if state.sidebar_collapsed {
        column![
            collapse_btn,
            rule::horizontal(1),
            soundboard_btn,
            settings_btn,
            Space::new().width(Length::Fill).height(Length::Fill),
            rule::horizontal(1),
            footer,
        ]
        .padding(10)
        .spacing(10)
        .align_x(iced::Alignment::Center)
    } else {
        column![
            row![
                text("Sound App").size(18),
                Space::new().width(Length::Fill),
                collapse_btn,
            ]
            .align_y(alignment::Vertical::Center),
            rule::horizontal(1),
            soundboard_btn,
            settings_btn,
            Space::new().width(Length::Fill).height(Length::Fill),
            rule::horizontal(1),
            footer,
        ]
        .padding(15)
        .spacing(10)
    };

    container(content)
        .width(sidebar_width)
        .height(Length::Fill)
        .into()
}
