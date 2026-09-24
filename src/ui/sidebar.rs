use iced::{Element, Length, Padding};

enum SidebarMode {
    EXPANDED,
    COLLAPSED,
}

struct Sidebar<'a, Message, Theme = iced::Theme, Renderer = iced::Renderer> {
    pub width: Length,
    pub height: Option<Length>,
    pub padding: Padding,
    pub collaped: bool,
    pub mode: SidebarMode,
    pub content: Element<'a, Message, Theme, Renderer>,
    pub footer: Element<'a, Message, Theme, Renderer>,
    pub collapse_button: Element<'a, Message, Theme, Renderer>,
}

impl<'a, Message, Theme, Renderer> Sidebar<'a, Message, Theme, Renderer>
where
    Renderer: iced_core::Renderer,
    Theme: iced::theme::Base,
{
}
