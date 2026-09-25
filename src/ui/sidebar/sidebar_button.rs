// use crate::{State, app::Message};
// use iced::Element;
// use iced::widget::{button, row, text};

// pub fn view(
//     state: &State,
//     collapsed_icon: lucide_icons::Icon,
//     expanded_icon: lucide_icons::Icon,
//     label: String,
//     on_press: Message,
// ) -> Element<'static, Message> {
//     let active_icon = if state.sidebar_collapsed {
//         collapsed_icon
//     } else {
//         expanded_icon
//     };

//     let icon_element: Element<'static, Message> = active_icon.into();

//     button(row![icon_element, text(label)].spacing(10))
//         .on_press(on_press)
//         .into()
// }
