use std::fs::File;
use std::io::BufReader;
use std::path::Path;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args();
    let Some(path) = args.nth(1) else {
        eprintln!("Usage: extract <path-to-qrc-file>");
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "missing argument"));
    };
    let file = File::open(&path)?;

    let parsed = qrc_parse::AnimFile::parse(BufReader::new(file))?;
    for file in parsed.files() {
        println!("{} ({} bytes)", file.path(), file.data().len());
    }

    Ok(())
}
