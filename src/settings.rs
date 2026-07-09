use std::path::PathBuf;

use bevy::prelude::*;
use bevy::window::{WindowCreated, WindowLevel};

pub fn init(application: &mut App) {
    application
        .insert_resource(match Settings::from_args() {
            Ok(settings) => settings,
            Err(e) => {
                tracing::error!("failed to parse CLI arguments: {e}");
                Settings::default()
            }
        })
        .add_systems(Update, self::apply_settings_to_existing_windows)
        .add_systems(Update, self::apply_settings_to_new_windows);
}

#[expect(clippy::needless_pass_by_value, reason = "`Res` is used for querying")]
fn apply_settings_to_existing_windows(settings: Res<Settings>, windows: Query<&mut Window>) {
    if !settings.is_changed() {
        return;
    }

    tracing::trace!("applying settings to windows: {:?}", settings.as_ref());

    for mut window in windows {
        settings.apply(&mut window);
    }
}

#[expect(clippy::needless_pass_by_value, reason = "`Res` is used for querying")]
fn apply_settings_to_new_windows(
    settings: Res<Settings>,
    mut new_window_reader: PopulatedMessageReader<WindowCreated>,
    mut windows: Query<&mut Window>,
) {
    tracing::trace!("applying settings to windows: {:?}", settings.as_ref());

    for window in new_window_reader.read() {
        tracing::trace!("applying settings to newly created window (ID {})", window.window);

        let Ok(mut window) = windows.get_mut(window.window) else {
            tracing::error!("newly created window (ID {}) missing `Window` component", window.window);
            continue;
        };
        settings.apply(&mut window);
    }
}

#[derive(Debug, bevy::ecs::resource::Resource)]
pub struct Settings {
    pub window_level: WindowLevel,
    pub msi_path: PathBuf,
}

impl Settings {
    pub fn from_args() -> Result<Self, String> {
        let mut out = Self::default();

        let args = std::env::args().collect::<Box<[String]>>();
        let args = args.iter().skip(1).map(String::as_str);

        let mut parser = carp::Parser::<&str, _>::new(args);
        let mut waiting_on: Option<PossibleSettings> = None;
        while let Some(parsed) = parser.parse_next().map_err(|e| format!("failed to parse argument: {e}"))? {
            match parsed {
                carp::ArgumentOrPositional::Argument(argument) => {
                    if let Some(waiting_on) = waiting_on
                        && waiting_on.must_take_value()
                    {
                        return Err(format!("missing value for argument '{waiting_on}'"));
                    }

                    waiting_on = argument.try_into().map(Some)?;
                }
                carp::ArgumentOrPositional::Positional(positional) => {
                    let mut setting = waiting_on.take().ok_or_else(|| format!("unexpected argument '{positional}'"))?;
                    setting.set_value_from_positional(positional)?;
                    out.assign(setting);
                }
            }
        }

        if let Some(setting) = waiting_on {
            if setting.must_take_value() {
                return Err(format!("missing value for argument '{setting}'"));
            }

            out.assign(setting);
        };

        Ok(out)
    }

    fn assign(&mut self, setting: PossibleSettings) {
        match setting {
            PossibleSettings::AlwaysOnTop(v) => {
                self.window_level = if v { WindowLevel::AlwaysOnTop } else { WindowLevel::Normal };
            }
            PossibleSettings::MsiPath(v) => self.msi_path = v,
        }
    }

    const fn apply(&self, window: &mut Window) {
        window.window_level = self.window_level;
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self { window_level: WindowLevel::Normal, msi_path: "./ChomikBox.msi".into() }
    }
}

enum PossibleSettings {
    AlwaysOnTop(bool),
    MsiPath(PathBuf),
}

impl PossibleSettings {
    const fn must_take_value(&self) -> bool {
        matches!(self, Self::MsiPath(_))
    }

    fn set_value_from_positional(&mut self, positional: &str) -> Result<(), String> {
        match self {
            Self::AlwaysOnTop(v) => {
                *v = positional
                    .parse()
                    .map_err(|_| format!("failed to parse argument '{positional}' (expected 'true' or 'false')"))?;
            }
            Self::MsiPath(v) => *v = positional.into(),
        }

        Ok(())
    }
}

impl TryFrom<carp::Argument<&str>> for PossibleSettings {
    type Error = String;

    fn try_from(argument: carp::Argument<&str>) -> Result<Self, Self::Error> {
        use carp::Argument::{Long, Short};

        Ok(match argument {
            Long("always-on-top") => Self::AlwaysOnTop(true),
            Long("msi-path") | Short('p') => Self::MsiPath(PathBuf::new()),
            arg => return Err(format!("unrecognized argument '{arg}'")),
        })
    }
}

impl std::fmt::Display for PossibleSettings {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", match self {
            Self::AlwaysOnTop(_) => "--always-on-top",
            Self::MsiPath(_) => "--msi-path",
        })
    }
}
