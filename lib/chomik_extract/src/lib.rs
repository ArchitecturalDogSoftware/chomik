//! # `chomik_extract`
//!
//! `chomik_extract` is a extractor for [ChomikBox](https://chomikuj.pl/chomikbox) MSI files.
//!
//! ## Examples
//!
//! ```no_run
#![doc = include_str!("../examples/extract.rs")]
//! ```

use std::io::{Read, Result, Seek};

mod msi;

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
