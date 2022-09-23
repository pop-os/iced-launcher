mod components;
#[rustfmt::skip]
mod config;
mod localize;
mod launcher_subscription;

use config::APP_ID;
use log::info;

use localize::localize;

use crate::{
    components::app,
    config::{PROFILE, VERSION},
};

fn main() -> iced::Result {
    // Initialize logger
    pretty_env_logger::init();
    info!("Iced Launcher ({})", APP_ID);
    info!("Version: {} ({})", VERSION, PROFILE);

    // Prepare i18n
    localize();

    app::run()
}
