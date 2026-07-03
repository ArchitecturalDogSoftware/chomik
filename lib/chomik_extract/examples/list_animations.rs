use std::fs::File;
use std::io::BufReader;

fn main() -> std::io::Result<()> {
    let mut args = std::env::args();
    let Some(path) = args.nth(1) else {
        eprintln!("Usage: list_animations <path-to-msi-file>");
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "missing argument"));
    };
    let msi = BufReader::new(File::open(&path)?);

    for anim_file in chomik_extract::extract_anims(msi)? {
        chomik_extract::print_animation_dbg_info(anim_file).unwrap();
    }

    Ok(())
}
