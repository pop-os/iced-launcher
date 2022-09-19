use adw_user_colors_lib::notify::*;
use iced::alignment::{Horizontal, Vertical};
use iced::futures::{channel::mpsc, SinkExt};
use iced::theme::palette::Extended;
use iced::theme::Palette;
use iced::widget::{button, column, container, row, scrollable, text, text_input, vertical_space};
use iced::{executor, Application, Command, Element, Length, Settings, Subscription, Theme, subscription, window};
use iced_native::widget::helpers;
use pop_launcher::SearchResult;

use crate::config;
use crate::wayland_subscription::{LauncherEvent, LauncherRequest, launcher};

pub const NUM_LAUNCHER_ITEMS: u8 = 10;

pub fn run() -> iced::Result {
    let mut settings = Settings::default();
    settings.window.decorations = false;
    settings.window.decorations = false;
    settings.window.size = (500, 100);
    IcedLauncher::run(settings)
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum ThemeType {
    Light,
    Dark,
    Custom,
}

#[derive(Default, Clone)]
struct IcedLauncher {
    tx: Option<mpsc::Sender<LauncherRequest>>,
    theme: Theme,
    input_value: String,
    launcher_items: Vec<SearchResult>,
    selected_item: Option<usize>,
}

#[derive(Debug, Clone)]
enum Message {
    PaletteChanged(Palette),
    InputChanged(String),
    Activate(Option<usize>),
    Select(Option<usize>),
    Clear,
    LauncherEvent(LauncherEvent),
    SentRequest,
    Error(String),
}

impl Application for IcedLauncher {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (IcedLauncher::default(), Command::none())
    }

    fn title(&self) -> String {
        config::APP_ID.to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
            Message::PaletteChanged(palette) => {
                self.theme = Theme::Custom {
                    palette,
                    extended: Extended::generate(palette),
                }
            }
            Message::InputChanged(value) => {
                self.input_value = value.clone();
                if let Some(tx) = self.tx.as_ref() {
                    let mut tx = tx.clone();
                    let cmd = async move { tx.send(LauncherRequest::Search(value)).await };
                    return Command::perform(cmd, |res| match res {
                        Ok(_) => Message::SentRequest,
                        Err(err) => Message::Error(err.to_string()),
                    });
                }
            },
            Message::Activate(Some(i)) => {
                if let (Some(tx), Some(item)) = (self.tx.as_ref(), self.launcher_items.get(i)) {
                    let mut tx = tx.clone();
                    let id = item.id;
                    let cmd = async move { tx.send(LauncherRequest::Activate(id)).await };
                    return Command::perform(cmd, |res| match res {
                        Ok(_) => Message::SentRequest,
                        Err(err) => Message::Error(err.to_string()),
                    });
                }
            }
            Message::Activate(None) => {
                if let (Some(tx), Some(item)) = (
                    self.tx.as_ref(),
                    self.launcher_items.get(self.selected_item.unwrap_or_default()),
                ) {
                    let mut tx = tx.clone();
                    let id = item.id;
                    let cmd = async move { tx.send(LauncherRequest::Activate(id)).await };
                    return Command::perform(cmd, |res| match res {
                        Ok(_) => Message::SentRequest,
                        Err(err) => Message::Error(err.to_string()),
                    });
                }
            }
            Message::LauncherEvent(e) => match e {
                LauncherEvent::Started(tx) => {
                    let mut tx_clone = tx.clone();
                    let cmd =
                        async move { tx_clone.send(LauncherRequest::Search("".to_string())).await };
                    self.tx.replace(tx);
                    // TODO send the thing as a command
                    return Command::perform(cmd, |res| match res {
                        Ok(_) => Message::SentRequest,
                        Err(err) => Message::Error(err.to_string()),
                    });
                }
                LauncherEvent::Response(response) => {
                    match response {
                        pop_launcher::Response::Close => todo!(),
                        pop_launcher::Response::Context { id, options } => todo!(),
                        pop_launcher::Response::DesktopEntry { path, gpu_preference } => todo!(),
                        pop_launcher::Response::Update(list) => {
                            self.launcher_items.splice(.., list);
                            let unit = 48;
                            let w = 500;
                            return window::resize(w, 100 + unit * self.launcher_items.len() as u32);

                        },
                        pop_launcher::Response::Fill(_) => todo!(),
                    }
                },
                LauncherEvent::Error(err) => {
                    log::error!("{}", err);
                }
            },
            Message::Clear => {
                self.input_value.clear();
                self.launcher_items.clear();
                if let Some(tx) = self.tx.as_ref() {
                    let mut tx = tx.clone();
                    let cmd = async move { tx.send(LauncherRequest::Search("".to_string())).await };
                    return Command::perform(cmd, |res| match res {
                        Ok(_) => Message::SentRequest,
                        Err(err) => Message::Error(err.to_string()),
                    });
                }
            }
            Message::SentRequest => {}
            Message::Error(err) => {
                log::error!("{}", err);
            }
            Message::Select(i) => {
                self.selected_item = i;
            }
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let text_input = text_input(
            "Type something...",
            &self.input_value,
            Message::InputChanged,
        )
        .on_submit(Message::Activate(None))
        .padding(10)
        .size(20);

        let clear_button = button("X").padding(10).on_press(Message::Clear);

        let buttons = self
            .launcher_items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let btn = button(
                    text(item.name.to_string())
                        .horizontal_alignment(Horizontal::Center)
                        .vertical_alignment(Vertical::Center)
                        .width(Length::Fill)
                        .height(Length::Fill),
                )
                .width(Length::Fill)
                .height(Length::Fill)
                .on_press(Message::Activate(Some(i)))
                .padding(0);
                btn.into()
            })
            .collect();

        let content = column![
            row![text_input, clear_button].spacing(10),
            helpers::column(buttons).spacing(8),
        ]
        .spacing(20)
        .padding(20)
        .max_width(600);

        container(content)
            .width(Length::Fill)
            .height(Length::Fill)
            .center_x()
            .center_y()
            .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(
            vec![
                theme(0).map(|(_, theme_update)| match theme_update {
                    ThemeUpdate::Palette(palette) => Message::PaletteChanged(palette),
                    ThemeUpdate::Errored => Message::Error("Theme update error!".to_string()),
                }),
                launcher(0).map(|(_, msg)| Message::LauncherEvent(msg)),
            ]
            .into_iter(),
        )
    }

    fn theme(&self) -> Theme {
        self.theme
    }
}
