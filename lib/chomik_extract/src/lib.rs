//! # `chomik_extract`
//!
//! `chomik_extract` is a extractor for [ChomikBox](https://chomikuj.pl/chomikbox) MSI files.
//!
//! ## Examples
//!
//! ```no_run
#![doc = include_str!("../examples/extract.rs")]
//! ```

use std::collections::HashMap;
use std::io::{Read, Seek};
use std::rc::Rc;
use std::time::Duration;

mod msi;
mod xml;

/// An `.anim` file found within a ChomikBox installer and its extracted contents.
#[derive(Debug, Hash, PartialEq, Eq)]
pub struct AnimFile {
    filename: Box<str>,
    parsed: qrc_parse::QrcFile,
}

impl AnimFile {
    /// The filename of the file.
    ///
    /// If present, this is the long name of the file within the MSI. Otherwise, it is the short name.
    ///
    /// This likely ends in `.anim`, but that isn't guaranteed. The only criteria that is checked is that it is within
    /// the `ANIMDIR` directory in the MSI.
    #[must_use]
    pub fn filename(&self) -> &str {
        &self.filename
    }

    /// The files embedded within this file, exposed as a flattened list.
    #[must_use]
    pub fn files(&self) -> Box<[qrc_parse::File<'_>]> {
        self.parsed.files()
    }
}

/// Extract every file in the `ANIMDIR` directory of the given MSI file and extract the files embedded within.
///
/// # Errors
///
/// Returns an error if [reading][`Read`] or [seeking][`Seek`] the input fails, if the input is not a valid MSI file, or
/// if a file in the `ANIMDIR` directory is not a valid Qt resource file.
pub fn extract_anims<R: Seek + Read>(msi: R) -> std::io::Result<Box<[AnimFile]>> {
    let (anim_files, mut cabinets) = msi::extract_anims(msi)?;

    let mut parsed = Vec::new();
    for file in anim_files {
        parsed.push(AnimFile {
            filename: file.filename().into(),
            parsed: qrc_parse::QrcFile::parse(cabinets.get_file(&file)?.into_reader())?,
        });
    }

    Ok(parsed.into_boxed_slice())
}

#[derive(Clone)]
pub struct Jpeg {
    /// E.g., `hamster_1639.a.jpg`.
    pub name: Box<str>,
    pub data: Rc<[u8]>,
}

impl std::fmt::Debug for Jpeg {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Jpeg").field("name", &self.name).finish_non_exhaustive()
    }
}

#[derive(Clone)]
pub struct Image {
    /// E.g., `hamster_1639`.
    pub asset_name: Box<str>,
    pub color: Jpeg,
    pub alpha: Jpeg,
}

impl std::fmt::Debug for Image {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Image").field("asset_name", &self.asset_name).finish_non_exhaustive()
    }
}

#[derive(Debug)]
pub struct Sequence {
    pub name: Box<str>,
    pub start: xml::State,
    pub stop: xml::State,
    pub images: Box<[Image]>,
}

pub struct AnimationSet {
    /// Triggered by the application being clicked by the mouse, intended to animate dragging the window around.
    pub mouse_press: Animation,
    /// Triggered by a file being dragged over the application.
    ///
    /// Should loop until the file is either no longer over the application or has been dropped on the application. If
    /// the file was dropped on the application, the looping portion should be allowed to finish, then the exit portion
    /// is skipped immediately enter the [file drop][`Self::file_drop`] animation.
    pub file_over: Animation,
    /// Triggered by a file being dropped on the application.
    pub file_drop: Animation,
    /// Triggered when the application begins to play music.
    pub music_playing: Animation,
    /// Triggered when a screenshot is taken.
    pub screenshot: Animation,
    /// Triggered when the user is typing.
    pub typing: Animation,
    /// One of many idle animations.
    pub idle: Box<[(u64, Animation)]>,
    /// The primary idle state that the application returns to afgter completing other animations. This animation
    /// ending triggers an [idle][`Self::idle`] animation on a weighted random basis.
    pub main_idle: (Duration, Animation),
}

impl AnimationSet {
    pub fn extract_from_msi<R: Seek + Read>(msi: R) -> std::io::Result<Self> {
        Ok(Self::from_typed_list(
            self::extract_anims(msi)?
                .into_iter()
                .map(<TypedAnimation as TryFrom<AnimFile>>::try_from)
                .collect::<Result<Box<[TypedAnimation]>, _>>()
                .unwrap(),
        )
        .unwrap())
    }

    fn from_typed_list(animations: impl IntoIterator<Item = TypedAnimation>) -> Result<Self, ()> {
        let mut mouse_press = None;
        let mut file_over = None;
        let mut file_drop = None;
        let mut music_playing = None;
        let mut screenshot = None;
        let mut typing = None;
        let mut idle = Vec::new();
        let mut main_idle = None;

        for TypedAnimation { trigger, animation } in animations {
            match trigger {
                AnimationTrigger::MousePress => mouse_press = Some(animation),
                AnimationTrigger::FileOver => file_over = Some(animation),
                AnimationTrigger::FileDrop => file_drop = Some(animation),
                AnimationTrigger::MusicPlaying => music_playing = Some(animation),
                AnimationTrigger::Screenshot => screenshot = Some(animation),
                AnimationTrigger::Typing => typing = Some(animation),
                AnimationTrigger::Idle { probability } => idle.push((probability, animation)),
                AnimationTrigger::MainIdle { duration } => main_idle = Some((duration, animation)),
            }
        }

        Ok(Self {
            mouse_press: mouse_press.unwrap(),
            file_over: file_over.unwrap(),
            file_drop: file_drop.unwrap(),
            music_playing: music_playing.unwrap(),
            screenshot: screenshot.unwrap(),
            typing: typing.unwrap(),
            // TO-DO: check for empty?
            idle: idle.into_boxed_slice(),
            main_idle: main_idle.unwrap(),
        })
    }
}

pub enum Animation {
    OneShot(OneShotAnimation),
    Looping(LoopingAnimation),
}

impl TryFrom<AnimFile> for Animation {
    type Error = ();

    fn try_from(extract_from: AnimFile) -> Result<Self, Self::Error> {
        TypedAnimation::try_from(extract_from).map(|TypedAnimation { animation, .. }| animation)
    }
}

pub struct OneShotAnimation {
    pub name: Box<str>,
    pub sequence: Sequence,
}

pub struct LoopingAnimation {
    pub name: Box<str>,
    pub entrance: Sequence,
    // The conditions of this sequence probably control triggering of the whole animation --- consider `AnimTyping`,
    // which has the `typing` condition set to be true, but `AnimTypingStart` doesn't (but would logically need to be
    // triggered by typing).
    pub looping: Sequence,
    pub exit: Sequence,
}

pub struct TypedAnimation {
    trigger: AnimationTrigger,
    animation: Animation,
}

pub enum AnimationTrigger {
    /// Triggered by the application being clicked by the mouse, intended to animate dragging the window around.
    MousePress,
    /// Triggered by a file being dragged over the application.
    ///
    /// Should loop until the file is either no longer over the application or has been dropped on the application. If
    /// the file was dropped on the application, the looping portion should be allowed to finish, then the exit portion
    /// is skipped immediately enter the [file drop][`Self::FileDrop`] animation.
    FileOver,
    /// Triggered by a file being dropped on the application.
    FileDrop,
    /// Triggered when the application begins to play music.
    MusicPlaying,
    /// Triggered when a screenshot is taken.
    Screenshot,
    /// Triggered when the user is typing.
    Typing,
    /// One of many idle animations.
    Idle { probability: u64 },
    /// The primary idle state that the application returns to afgter completing other animations. This animation ending
    /// triggers an [idle][`Self::Idle`] animation on a weighted random basis.
    MainIdle { duration: Duration },
}

impl AnimationTrigger {
    fn detect(conditions: &xml::Conditions, way: &xml::Way) -> Option<Self> {
        // TO-DO: should these also check that certain conditions are NOT true?
        if conditions.mouse_press.is_some_and(|v| v) {
            Some(Self::MousePress)
        } else if conditions.file_over.is_some_and(|v| v) {
            Some(Self::FileOver)
        } else if conditions.file_drop.is_some_and(|v| v) {
            Some(Self::FileDrop)
        } else if conditions.player_playing.is_some_and(|v| v) {
            Some(Self::MusicPlaying)
        } else if conditions.screenshot.is_some_and(|v| v) {
            Some(Self::Screenshot)
        } else if conditions.typing.is_some_and(|v| v) {
            Some(Self::Typing)
        } else if conditions.idle.is_some_and(|v| v)
            && let Some(probability) = way.prob
        {
            Some(Self::Idle { probability })
        } else if conditions.idle.is_some_and(|v| v)
            && way.prob.is_none()
            // I don't actually know that it's milliseconds for sure, but that seems reasonable based on what I
            // observed.
            && let Some(duration_ms) = conditions.duration
        {
            Some(Self::MainIdle { duration: Duration::from_millis(duration_ms) })
        } else {
            None
        }
    }
}

// TO-DO: there may be multiple per `.anim` file.
impl TryFrom<AnimFile> for TypedAnimation {
    type Error = ();

    fn try_from(value: AnimFile) -> Result<Self, Self::Error> {
        let xml::AnimContents { name, animations, mut jpegs } = xml::extract_files(&value)?;

        let (entrance, looping, exit) = animations
            .into_iter()
            .map(|animation| {
                let mut fetch_jpeg = |filename: Box<str>| Jpeg {
                    data: jpegs.get_mut(filename.as_ref()).unwrap().clone(),
                    name: filename,
                };

                let sequence = Sequence {
                    name: animation.name,
                    start: animation.way.start.unwrap(), // Seemingly always present.
                    stop: animation.way.stop.unwrap(),   // Seemingly always present.
                    images: animation
                        .files
                        .into_iter()
                        .map(|asset_name: Box<str>| Image {
                            color: fetch_jpeg(format!("{asset_name}.p.jpg").into_boxed_str()),
                            alpha: fetch_jpeg(format!("{asset_name}.a.jpg").into_boxed_str()),
                            asset_name,
                        })
                        .collect(),
                };

                (animation.conditions, animation.way, sequence)
            })
            .fold((None, None, None), |(entrance, middle, exit), animation| {
                match (animation.1.enter.unwrap_or(false), animation.1.exit.unwrap_or(false)) {
                    (true, true) | (false, false) => (entrance, Some(animation), exit),
                    (true, false) => (Some(animation), middle, exit),
                    (false, true) => (entrance, middle, Some(animation)),
                }
            });

        let (conditions, way, animation) = match (entrance, looping, exit) {
            (None, Some((conditions, way, sequence)), None) => {
                (conditions, way, Animation::OneShot(OneShotAnimation { name, sequence }))
            }
            (Some((_, _, entrance)), Some((conditions, way, looping)), Some((_, _, exit))) => {
                (conditions, way, Animation::Looping(LoopingAnimation { name, entrance, looping, exit }))
            }
            other => panic!("{other:?}"),
            // _ => return Err(()),
        };
        let trigger = AnimationTrigger::detect(&conditions, &way).unwrap();
        Ok(Self { trigger, animation })
    }
}

pub fn print_animation_dbg_info(extract_from: AnimFile) -> Result<(), ()> {
    const LEFT_DASHES: &str = "-----------------";
    const ANSI_BLUE: &str = "\u{001B}[38;5;12m";
    const ANSI_GRAY: &str = "\u{001B}[38;5;244m";
    const ANSI_RESET: &str = "\u{001B}[0m";

    let xml::AnimContents { name, animations, jpegs: _ } = xml::extract_files(&extract_from)?;

    // 18 is the widest filename observed.
    let right_dashes = "-".repeat(LEFT_DASHES.len() + 18 - extract_from.filename().len());
    println!(
        // 18 is the widest name observed.
        "{name:18} {ANSI_BLUE}{LEFT_DASHES}(Source: {ANSI_GRAY}{}{ANSI_BLUE}){right_dashes}{ANSI_RESET}",
        extract_from.filename()
    );

    for xml_anim in &animations {
        // 36 is the widest `conditions.dbg_inline()` observed.
        let cond_spaces = " ".repeat(36_usize.saturating_sub(xml_anim.conditions.dbg_inline().len()));

        println!(
            // 31 is the widest name observed.
            "    {:31} {}{cond_spaces} {}",
            xml_anim.name,
            xml_anim.conditions.dbg_inline_clr(),
            xml_anim.way.dbg_inline_clr(),
        );
    }

    println!();

    Ok(())
}
