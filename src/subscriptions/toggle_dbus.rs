use futures::{
    channel::mpsc::{unbounded, UnboundedReceiver, UnboundedSender},
    StreamExt,
};
use iced::subscription;
use std::{fmt::Debug, hash::Hash};
use zbus::{dbus_interface, Connection, ConnectionBuilder};

pub fn dbus_toggle<I: 'static + Hash + Copy + Send + Sync + Debug>(
    id: I,
) -> iced::Subscription<(I, DbusEvent)> {
    subscription::unfold(id, State::Ready, move |state| start_listening(id, state))
}

#[derive(Debug, Clone)]
pub enum DbusEvent {
    Started,
    Toggle,
}

pub enum State {
    Ready,
    Waiting(Connection, UnboundedReceiver<LauncherDbusEvent>),
    Finished,
}

async fn start_listening<I: Copy>(id: I, state: State) -> (Option<(I, DbusEvent)>, State) {
    match state {
        State::Ready => {
            println!("creating conn");
            let (tx, rx) = unbounded();
            if let Some(conn) = ConnectionBuilder::session()
                .ok()
                .and_then(|conn| conn.name(crate::config::APP_ID).ok())
                .and_then(|conn| {
                    conn.serve_at("/com/system76/IcedLauncher", IcedLauncherServer { tx })
                        .ok()
                })
                .and_then(|conn| conn.name(crate::config::APP_ID).ok())
                .map(|conn| conn.build())
            {
                if let Ok(conn) = conn.await {
                    return (Some((id, DbusEvent::Started)), State::Waiting(conn, rx));
                }
            }
            return (None, State::Finished);
        }
        State::Waiting(conn, mut rx) => {
            println!("waiting");
            if let Some(LauncherDbusEvent::Toggle) = rx.next().await {
                println!("Toggling");
                (Some((id, DbusEvent::Toggle)), State::Waiting(conn, rx))
            } else {
                (None, State::Finished)
            }
        }
        State::Finished => iced::futures::future::pending().await,
    }
}

#[derive(Debug, Clone, Copy)]
pub enum LauncherDbusEvent {
    Toggle,
}

#[derive(Debug)]
pub(crate) struct IcedLauncherServer {
    pub(crate) tx: UnboundedSender<LauncherDbusEvent>,
}

#[dbus_interface(name = "com.system76.IcedLauncher")]
impl IcedLauncherServer {
    async fn toggle(&self) {
        println!("sending toggle");

        let _ = self.tx.unbounded_send(LauncherDbusEvent::Toggle);
    }
}
