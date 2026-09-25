// src/app.rs
use iced::widget::row;
use iced::{Element, Length};

use crate::audio::player::AudioPlayer;
use crate::model::config::Config;
use crate::model::sound::SoundEntry;
use crate::ui::sidebar::sidebar_panel;
use crate::ui::view::soundboard_page;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Soundboard,
    Settings,
}

pub struct State {
    pub config: Config,
    pub player: AudioPlayer,
    pub sidebar_collapsed: bool,
    pub active_tab: Tab,
}

impl Default for State {
    fn default() -> Self {
        Self {
            config: Config::default(),
            player: AudioPlayer::new(),
            sidebar_collapsed: false,
            active_tab: Tab::Soundboard,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Message {
    ToggleSidebar,
    SwitchTab(Tab),
    PlaySound(u32),
    StopSound(u32),
    StopAll,
    AddSoundClicked,
    RemoveSound(u32),
}

impl State {
    pub fn update(&mut self, message: Message) {
        match message {
            Message::ToggleSidebar => {
                self.sidebar_collapsed = !self.sidebar_collapsed;
            }
            Message::SwitchTab(tab) => {
                self.active_tab = tab;
            }
            Message::PlaySound(id) => {
                if let Some(sound) = self.config.sounds.iter().find(|s| s.id == id) {
                    self.player.play(sound);
                }
            }
            Message::StopSound(id) => self.player.stop(id),
            Message::StopAll => self.player.stop_all(),
            Message::AddSoundClicked => {
                if let Some(path) = rfd::FileDialog::new()
                    .add_filter("audio", &["mp3", "wav", "ogg", "flac"])
                    .pick_file()
                {
                    let id = self.config.next_id;
                    self.config.next_id += 1;

                    let name = path
                        .file_stem()
                        .map(|s| s.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Unnamed".into());

                    self.config.sounds.push(SoundEntry {
                        id,
                        name,
                        file_path: path,
                        volume: 1.0,
                        keybind: None,
                    });
                }
            }
            Message::RemoveSound(id) => {
                self.config.sounds.retain(|s| s.id != id);
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let sidebar_element = sidebar_panel::view(self);
        let main_element = soundboard_page::view(self);

        // Fuse sidebar and main content side-by-side using a Row layout
        row![
            sidebar_element,
            iced::widget::rule::vertical(1), // Optional divider line
            main_element,
        ]
        .width(Length::Fill)
        .height(Length::Fill)
        .into()
    }
}
