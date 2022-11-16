use cosmic::iced::alignment::{Horizontal, Vertical};
use cosmic::iced::futures::{channel::mpsc, SinkExt};
use cosmic::iced::subscription::events_with;
use cosmic::iced::widget::text_input::Id;
use cosmic::iced::widget::{button, column, container, row, text, text_input};
use cosmic::iced::{executor, Application, Command, Length, Subscription};
use cosmic::iced_native::widget::helpers;
use cosmic::iced_native::window::Id as SurfaceId;
use cosmic::iced_style::{application};
use cosmic::theme::Container;
use cosmic::widget::icon;
use cosmic::{settings, widget, Element, Theme};
use iced::wayland::Appearance;
use iced::Color;
use iced::widget::svg;
use iced_sctk::application::SurfaceIdWrapper;
use iced_sctk::command::platform_specific::wayland::layer_surface::SctkLayerSurfaceSettings;
use iced_sctk::commands;
use iced_sctk::commands::layer_surface::{Anchor, KeyboardInteractivity, Layer};
use iced_sctk::event::wayland::LayerEvent;
use iced_sctk::event::{wayland, PlatformSpecific};
use iced_sctk::settings::InitialSurface;
use pop_launcher::{SearchResult, IconSource};
use xdg::BaseDirectories;

use crate::config;
use crate::subscriptions::launcher::{launcher, LauncherEvent, LauncherRequest};
use crate::subscriptions::toggle_dbus::dbus_toggle;

pub const NUM_LAUNCHER_ITEMS: u8 = 10;

pub fn run() -> cosmic::iced::Result {
    let mut settings = settings();
    settings.initial_surface = InitialSurface::LayerSurface(SctkLayerSurfaceSettings {
        keyboard_interactivity: KeyboardInteractivity::None,
        namespace: "ignore".into(),
        size: (Some(1), Some(1)),
        layer: Layer::Background,
        ..Default::default()
    });
    IcedLauncher::run(settings.into())
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
enum ThemeType {
    Light,
    Dark,
    Custom,
}

#[derive(Default, Clone)]
struct IcedLauncher {
    id_ctr: u64,
    tx: Option<mpsc::Sender<LauncherRequest>>,
    input_value: String,
    launcher_items: Vec<SearchResult>,
    selected_item: Option<usize>,
    base_directories: Option<BaseDirectories>,
    active_surface: Option<SurfaceId>,
    theme: Theme,
}

#[derive(Debug, Clone)]
enum Message {
    InputChanged(String),
    Activate(Option<usize>),
    Select(Option<usize>),
    Clear,
    LauncherEvent(LauncherEvent),
    SentRequest,
    Error(String),
    Layer(LayerEvent),
    Toggle,
    Closed,
}

impl Application for IcedLauncher {
    type Message = Message;
    type Theme = Theme;
    type Executor = executor::Default;
    type Flags = ();

    fn new(_flags: ()) -> (Self, Command<Message>) {
        (
            IcedLauncher {
                base_directories: xdg::BaseDirectories::with_prefix("icons").ok(),
                ..Default::default()
            },
            Command::none(),
        )
    }

    fn title(&self) -> String {
        config::APP_ID.to_string()
    }

    fn update(&mut self, message: Message) -> Command<Message> {
        match message {
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
            Message::Layer(e) => match e {
                LayerEvent::Focused(_) => {}
                LayerEvent::Unfocused(_) => {
                    if let Some(id) = self.active_surface {
                        return commands::layer_surface::destroy_layer_surface(id);
                    }
                }
                _ => {}
            },
            Message::Closed => {
                self.active_surface.take();
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
            Message::Toggle => {
                if let Some(id) = self.active_surface {
                    return commands::layer_surface::destroy_layer_surface(id);
                } else {
                    self.id_ctr += 1;
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
                    cmds.push(commands::layer_surface::get_layer_surface(
                        SctkLayerSurfaceSettings {
                            id: SurfaceId::new(self.id_ctr),
                            keyboard_interactivity: KeyboardInteractivity::OnDemand,
                            anchor: Anchor::TOP.union(Anchor::BOTTOM),
                            namespace: "launcher".into(),
                            size: (Some(600), None),
                            ..Default::default()
                        },
                    ));
                    return Command::batch(cmds);
                }
            }
        }
        Command::none()
    }

    fn view(&self, id: SurfaceIdWrapper) -> Element<Message> {
        if id.inner() == SurfaceId::new(0) {
            // TODO just delete the original surface if possible
            return text("").into();
        }

        let launcher_entry = text_input(
            "Type something...",
            &self.input_value,
            Message::InputChanged,
        )
        .on_submit(Message::Activate(None))
        .padding(8)
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
                    .width(Length::Fill);
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
                    match icon_source {
                        IconSource::Name(name) => {
                            button_content.push(icon(name, 24).into());
                        },
                        IconSource::Mime(mime) => {
                            button_content.push(icon(mime, 24).into());
                        },
                    }
                }

                if let (Some(icon_source), Some(_base_dirs)) =
                    (item.icon.as_ref(), self.base_directories.as_ref())
                {
                    match icon_source {
                        IconSource::Name(name) => {
                            button_content.push(icon(name, 24).into());
                        },
                        IconSource::Mime(mime) => {
                            button_content.push(icon(mime, 24).into());
                        },
                    }
                }

                let description = text(description)
                    .horizontal_alignment(Horizontal::Left)
                    .vertical_alignment(Vertical::Center)
                    .width(Length::Fill);

                button_content.push(column![name, description].into());

                let btn = button(helpers::row(button_content))
                    .width(Length::Fill)
                    .on_press(Message::Activate(Some(i)))
                    .padding([8, 16]);

                btn.into()
            })
            .collect();

        let content = column![
            row![launcher_entry, clear_button].spacing(16),
            helpers::column(buttons).spacing(8),
        ]
        .spacing(16)
        .max_width(600);

        widget::widget::container(widget::widget::container(content).style(Container::Custom(
            |theme| container::Appearance {
                text_color: Some(theme.cosmic().on_bg_color().into()),
                background: Some(theme.extended_palette().background.base.color.into()),
                border_radius: 16.0,
                border_width: 0.0,
                border_color: Color::TRANSPARENT,
            },
        )).padding([16, 24]))
        .width(Length::Fill)
        .height(Length::Fill)
        .align_y(Vertical::Center)
        .into()
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(
            vec![
                launcher(0).map(|(_, msg)| Message::LauncherEvent(msg)),
                events_with(|e, _status| match e {
                    cosmic::iced::Event::PlatformSpecific(PlatformSpecific::Wayland(
                        wayland::Event::Layer(e),
                    )) => Some(Message::Layer(e)),
                    _ => None,
                }),
                dbus_toggle(1).map(|_| Message::Toggle),
            ]
            .into_iter(),
        )
    }

    fn style(&self) -> <Self::Theme as application::StyleSheet>::Style {
        <Self::Theme as application::StyleSheet>::Style::Custom(|theme| Appearance {
            background_color: Color::from_rgba(0.0, 0.0, 0.0, 0.0),
            text_color: theme.cosmic().on_bg_color().into(),
        })
    }

    fn theme(&self) -> Theme {
        self.theme
    }

    fn close_requested(&self, _id: iced_sctk::application::SurfaceIdWrapper) -> Self::Message {
        Message::Closed
    }
}
