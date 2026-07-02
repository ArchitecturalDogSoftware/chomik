use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::process::Command;

use chomik_extract::{Animation, Image, LoopingAnimation, OneShotAnimation, Sequence};

fn main() -> std::io::Result<()> {
    let mut args = std::env::args();
    let Some(path) = args.nth(1) else {
        eprintln!("Usage: list_animations <path-to-msi-file>");
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "missing argument"));
    };
    let file = File::open(&path)?;

    let extracted = chomik_extract::extract_anims(BufReader::new(file))?;
    for anim in extracted {
        println!("{}", anim.filename());
        let animation = Animation::try_from(anim).unwrap();
        println!("    \u{001B}[38;5;244mAnimation Name: {}\u{001B}[0m", match animation {
            Animation::OneShot(OneShotAnimation { name, .. }) | Animation::Looping(LoopingAnimation { name, .. }) =>
                name,
        });
    }

    Ok(())
}
