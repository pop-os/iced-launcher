use glob::glob;
use iced::{
    widget::{image, svg},
    Length,
};
use std::path::PathBuf;
use xdg::BaseDirectories;

pub fn svg_icon(
    theme: Option<&str>,
    icon_source: &pop_launcher::IconSource,
    width: u16,
    height: u16,
) -> Option<iced::widget::Svg> {
    let (name, base_dirs) = match icon_source {
        pop_launcher::IconSource::Name(name) => (name, BaseDirectories::with_prefix("icons")),
        pop_launcher::IconSource::Mime(name) => {
            let ret = if let Some(theme) = theme {
                (name, BaseDirectories::with_prefix(format!("icons/{theme}")))
            } else {
                (name, BaseDirectories::with_prefix("icons"))
            };
            ret
        }
    };

    base_dirs
        .ok()
        .map(|base_dirs| base_dirs.get_data_dirs())
        .and_then(|dirs| {
            dirs.iter().find_map(|dir| {
                glob(&format!(
                    "{}/**/scalable/**/{}.svg",
                    dir.to_string_lossy(),
                    name
                ))
                .ok()
                .and_then(|mut paths| paths.next().and_then(|p| p.ok()))
            })
        })
        .map(|path| {
            svg(svg::Handle::from_path(path))
                .width(Length::Units(width))
                .height(Length::Units(height))
        })
}

pub fn image_icon(
    theme: Option<&str>,
    icon_source: &pop_launcher::IconSource,
    width: u16,
    height: u16,
) -> Option<iced::widget::Image> {
    let mut path = PathBuf::new();

    let base_dirs = match icon_source {
        pop_launcher::IconSource::Name(name) => {
            path.push("apps");
            path.push(format!("{name}.png"));
            BaseDirectories::with_prefix("icons")
        }
        pop_launcher::IconSource::Mime(name) => {
            let ret = if let Some(theme) = theme {
                path.push(theme);
                BaseDirectories::with_prefix(format!("icons/{theme}"))
            } else {
                BaseDirectories::with_prefix("icons")
            };
            path.push(format!("{name}.png"));
            ret
        }
    };

    base_dirs
        .ok()
        .map(|base_dirs| base_dirs.find_data_files(path))
        .and_then(|iter| {
            let found: Vec<_> = iter.collect();
            if let Some(p) = found.iter().find(|p| {
                p.components()
                    .any(|dir| dir.as_os_str() == format!("{width}x{height}").as_str())
            }) {
                Some(p.clone())
            } else {
                // TODO sort found by decreasing order & take largest
                found.iter().next().cloned()
            }
        })
        .map(|path| {
            image(image::Handle::from_path(path))
                .width(Length::Units(width))
                .height(Length::Units(height))
        })
}
