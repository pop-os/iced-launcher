use adw_user_colors_lib::notify::*;
use iced::alignment::{Horizontal, Vertical};
use iced::futures::{channel::mpsc, SinkExt};
use iced::subscription::events_with;
use iced::theme::palette::Extended;
use iced::theme::Palette;
use iced::widget::text_input::Id;
use iced::widget::{button, column, container, row, text, text_input};
use iced::{
    executor, window, Application, Command, Element, Length, Settings, Subscription, Theme,
};
use iced_native::widget::helpers;
use once_cell::sync::OnceCell;
use pop_launcher::SearchResult;
use xdg::BaseDirectories;
use zbus::Connection;

use crate::config;
use crate::launcher_subscription::{launcher, LauncherEvent, LauncherRequest};
use crate::util::{image_icon, svg_icon};

pub const NUM_LAUNCHER_ITEMS: u8 = 10;

pub fn run() -> iced::Result {
    let mut settings = Settings::default();
    settings.window.decorations = false;
    settings.window.decorations = false;
    settings.window.size = (600, 120);
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
    dbus_conn: OnceCell<Connection>,
    base_directories: Option<BaseDirectories>,
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
    Window(iced_native::window::Event),
    DbusConn(Connection),
}

impl Application for IcedLauncher {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        let cmd = async move { Connection::session().await };
        (
            IcedLauncher {
                base_directories: xdg::BaseDirectories::with_prefix("icons").ok(),
                ..Default::default()
            },
            Command::perform(cmd, |res| match res {
                Ok(conn) => Message::DbusConn(conn),
                Err(err) => Message::Error(err.to_string()),
            }),
        )
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
            }
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
                    self.launcher_items
                        .get(self.selected_item.unwrap_or_default()),
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
                LauncherEvent::Response(response) => match response {
                    pop_launcher::Response::Close => todo!(),
                    pop_launcher::Response::Context { id, options } => todo!(),
                    pop_launcher::Response::DesktopEntry {
                        path,
                        gpu_preference,
                    } => todo!(),
                    pop_launcher::Response::Update(list) => {
                        self.launcher_items.splice(.., list);
                        let unit = 48;
                        let w = 600;
                        return window::resize(w, 100 + unit * self.launcher_items.len() as u32);
                    }
                    pop_launcher::Response::Fill(_) => todo!(),
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
            Message::Window(e) => match e {
                iced_native::window::Event::Focused => {
                    let mut cmds = Vec::new();
                    if let Some(tx) = self.tx.as_ref() {
                        let mut tx = tx.clone();
                        let search_cmd =
                            async move { tx.send(LauncherRequest::Search("".to_string())).await };
                        cmds.push(Command::perform(search_cmd, |res| match res {
                            Ok(_) => Message::SentRequest,
                            Err(err) => Message::Error(err.to_string()),
                        }));
                    }
                    self.input_value = "".to_string();
                    cmds.push(text_input::focus(Id::new("launcher_entry")));
                    return Command::batch(cmds);
                }
                iced_native::window::Event::Unfocused => {
                    let mut cmds = Vec::new();
                    if let Some(tx) = self.tx.as_ref() {
                        let mut tx = tx.clone();
                        let search_cmd =
                            async move { tx.send(LauncherRequest::Search("".to_string())).await };
                        cmds.push(Command::perform(search_cmd, |res| match res {
                            Ok(_) => Message::SentRequest,
                            Err(err) => Message::Error(err.to_string()),
                        }));
                    }
                    if let Some(dbus_conn) = self.dbus_conn.get() {
                        let dbus_conn = dbus_conn.clone();
                        let cmd = async move {
                            dbus_conn
                                .call_method(
                                    Some("com.system76.CosmicAppletHost"),
                                    "/com/system76/CosmicAppletHost",
                                    Some("com.system76.CosmicAppletHost"),
                                    "Hide",
                                    &("com.system76.IcedLauncher"),
                                )
                                .await
                        };
                        cmds.push(Command::perform(cmd, |res| match res {
                            Ok(_) => Message::SentRequest,
                            Err(err) => Message::Error(err.to_string()),
                        }));
                    }
                    self.input_value = "".to_string();
                    cmds.push(text_input::focus(Id::new("launcher_entry")));
                    return Command::batch(cmds);
                }
                _ => {}
            },
            Message::DbusConn(conn) => self.dbus_conn.set(conn).unwrap(),
        }
        Command::none()
    }

    fn view(&self) -> Element<Message> {
        let launcher_entry = text_input(
            "Type something...",
            &self.input_value,
            Message::InputChanged,
        )
        .on_submit(Message::Activate(None))
        .padding(10)
        .size(20)
        .id(Id::new("launcher_entry"));

        let clear_button = button("X").padding(10).on_press(Message::Clear);

        let buttons = self
            .launcher_items
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let name = text(item.name.to_string())
                    .horizontal_alignment(Horizontal::Left)
                    .vertical_alignment(Vertical::Center)
                    .width(Length::Fill)
                    .height(Length::Fill);
                let description = if item.description.len() > 40 {
                    format!(
                        "{}...",
                        item.description[0..45.min(item.description.len())].to_string()
                    )
                } else {
                    item.description.to_string()
                };

                let mut button_content = Vec::new();
                if let (Some(icon_source), Some(_base_dirs)) =
                    (item.category_icon.as_ref(), self.base_directories.as_ref())
                {
                    if let Some(icon) = svg_icon(None, icon_source, 32, 32) {
                        button_content.push(icon.into());
                    }
                }

                if let (Some(icon_source), Some(_base_dirs)) =
                    (item.icon.as_ref(), self.base_directories.as_ref())
                {
                    if let Some(icon) = svg_icon(None, icon_source, 32, 32) {
                        button_content.push(icon.into());
                    }
                }

                let description = text(description)
                    .horizontal_alignment(Horizontal::Left)
                    .vertical_alignment(Vertical::Center)
                    .width(Length::Fill)
                    .height(Length::Fill);

                button_content.push(column![name, description].into());

                let btn = button(helpers::row(button_content))
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .on_press(Message::Activate(Some(i)))
                    .padding(0);

                btn.into()
            })
            .collect();

        let content = column![
            row![launcher_entry, clear_button].spacing(10),
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
                events_with(|e, status| match e {
                    iced::Event::Window(e) => Some(Message::Window(e)),
                    _ => None,
                }),
            ]
            .into_iter(),
        )
    }

    fn theme(&self) -> Theme {
        self.theme
    }
}
