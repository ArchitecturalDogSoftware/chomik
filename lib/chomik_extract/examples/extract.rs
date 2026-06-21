use std::fs::File;
use std::io::BufReader;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args();
    let Some(path) = args.nth(1) else {
        eprintln!("Usage: extract <path-to-msi-file>");
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "missing argument"));
    };
    let file = File::open(&path)?;

    let extracted = chomik_extract::extract_anims(BufReader::new(file))?;
    for anim in extracted {
        println!("{}", anim.filename());
        for file in anim.files() {
            println!("    {} ({} bytes)", file.path(), file.raw_data().len());
        }
    }

    Ok(())
}
