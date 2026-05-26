use std::io::{Read, Result, Seek};

mod msi;

pub fn extract_anims<R: Seek + Read>(msi: R) -> Result<Box<[qrc_parse::AnimFile]>> {
    let (anim_files, mut cabinets) = msi::extract_anims(msi)?;

    let mut parsed = Vec::new();
    for file in anim_files {
        parsed.push(qrc_parse::AnimFile::parse(cabinets.get_file(&file)?.into_reader())?);
    }

    Ok(parsed.into_boxed_slice())
}
