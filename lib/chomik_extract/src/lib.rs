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
use std::io::{Read, Result, Seek};
use std::rc::Rc;

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
pub fn extract_anims<R: Seek + Read>(msi: R) -> Result<Box<[AnimFile]>> {
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
    pub way_probability: Option<u64>,
    pub images: Box<[Image]>,
}

pub enum Animation {
    OneShot(OneShotAnimation),
    Looping(LoopingAnimation),
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

const XML_MAGICS: [&[u8]; 2] = [b"<?xml", b"\xEF\xBB\xBF<?xml"];
const JPEG_MAGICS: [&[u8]; 3] = [
    [0xFF, 0xD8, 0xFF].as_slice(),
    [0x00, 0x00, 0x00, 0x0C, 0x6A, 0x50, 0x20, 0x20, 0x0D, 0x0A, 0x87, 0x0A].as_slice(),
    [0xFF, 0x4F, 0xFF, 0x51].as_slice(),
];

// TO-DO: there may be multiple per `.anim` file.
impl TryFrom<AnimFile> for Animation {
    type Error = ();

    fn try_from(value: AnimFile) -> std::result::Result<Self, Self::Error> {
        enum FileType {
            Jpeg,
            Xml,
        }

        let (xml, mut jpegs) = value
            .files()
            .into_iter()
            .map(|f| {
                let data = f.decompressed_data().unwrap();
                if XML_MAGICS.iter().any(|magic| data.starts_with(magic)) {
                    Ok((FileType::Xml, f.name(), data))
                } else if JPEG_MAGICS.iter().any(|magic| data.starts_with(magic)) {
                    Ok((FileType::Jpeg, f.name(), data))
                } else {
                    dbg!(data);
                    Err(())
                }
            })
            .try_fold((None, HashMap::<&str, Rc<[u8]>>::new()), |(mut xml, mut jpegs): (_, _), v| {
                let (ft, filename, data) = v?;
                match ft {
                    FileType::Jpeg => {
                        jpegs.insert(filename, Rc::from(data));
                    }
                    FileType::Xml => {
                        if let Some((prev_filename, _)) = xml {
                            panic!(
                                "`.anim` file contains multiple XML files (tried to overwrite '{prev_filename}' with \
                                 '{filename}')",
                            )
                        }
                        // println!("```\n{}\n```", str::from_utf8(data.as_ref()).unwrap());

                        xml = Some((filename, data));
                    }
                }
                Ok((xml, jpegs))
            })?;

        let (_, data) = xml.unwrap();
        let (name, animations) = xml::parse(data.as_ref()).unwrap();

        for anim in &animations {
            const COND_DBG_WIDTH: usize = 36;
            let spaces = " ".repeat(COND_DBG_WIDTH.saturating_sub(anim.conditions.dbg_inline().len()));
            println!("    {:31} {}{spaces} {}", anim.name, anim.conditions.dbg_inline_clr(), anim.way.dbg_inline_clr());
        }

        let (entrance, looping, exit) = animations
            .into_iter()
            .map(|animation| {
                let mut fetch_jpeg = |filename: Box<str>| Jpeg {
                    data: jpegs.get_mut(filename.as_ref()).unwrap().clone(),
                    name: filename,
                };
                let mut fetch_sequence = |animation: xml::Animation| Sequence {
                    name: animation.name,
                    start: animation.way.start.unwrap(), // Seemingly always present.
                    stop: animation.way.stop.unwrap(),   // Seemingly always present.
                    way_probability: animation.way.prob,
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

                (animation.way.enter.unwrap_or(false), animation.way.exit.unwrap_or(false), fetch_sequence(animation))
            })
            .fold((None, None, None), |(entrance, middle, exit), (is_entrance, is_exit, animation)| {
                match (is_entrance, is_exit) {
                    (true, true) | (false, false) => (entrance, Some(animation), exit),
                    (true, false) => (Some(animation), middle, exit),
                    (false, true) => (entrance, middle, Some(animation)),
                }
            });

        match (entrance, looping, exit) {
            (None, Some(sequence), None) => Ok(Self::OneShot(OneShotAnimation { name, sequence })),
            (Some(entrance), Some(looping), Some(exit)) => {
                Ok(Self::Looping(LoopingAnimation { name, entrance, looping, exit }))
            }
            o => panic!("{o:?}"),
            // _ => Err(()),
        }
    }
}
