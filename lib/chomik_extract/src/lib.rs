use std::io::{Read, Result, Seek};

mod msi;

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct AnimFile {
    filename: Box<str>,
    parsed: qrc_parse::QrcFile,
}

impl AnimFile {
    #[must_use]
    pub fn filename(&self) -> &str {
        &self.filename
    }

    #[must_use]
    pub fn files(&self) -> Box<[qrc_parse::File<'_>]> {
        self.parsed.files()
    }
}

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
