use crate::app::{Message, State, Tab};
use iced::widget::{button, column, container, row, scrollable, text};
use iced::{Element, Length};

pub fn view(state: &State) -> Element<Message> {
    let content: Element<Message> = match state.active_tab {
        Tab::Soundboard => {
            let sound_rows: Vec<Element<Message>> = state
                .config
                .sounds
                .iter()
                .map(|sound| {
                    row![
                        button(text(sound.name.clone())).on_press(Message::PlaySound(sound.id)),
                        button("stop").on_press(Message::StopSound(sound.id)),
                        button("x").on_press(Message::RemoveSound(sound.id)),
                    ]
                    .spacing(8)
                    .into()
                })
                .collect();

            column![
                row![
                    button("+ Add Sound").on_press(Message::AddSoundClicked),
                    button("Stop All").on_press(Message::StopAll),
                ]
                .spacing(8),
                scrollable(column(sound_rows).spacing(6)).height(Length::Fill),
            ]
            .spacing(16)
            .into()
        }
        Tab::Settings => column![
            text("Settings").size(24),
            text("Configure your audio devices or preferences here."),
        ]
        .spacing(10)
        .into(),
    };

    container(content)
        .width(Length::Fill)
        .height(Length::Fill)
        .padding(20)
        .into()
}
