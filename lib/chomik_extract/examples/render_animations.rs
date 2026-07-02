use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::process::Command;

use chomik_extract::{Animation, Image, LoopingAnimation, OneShotAnimation, Sequence};

fn main() -> std::io::Result<()> {
    let mut args = std::env::args();
    let Some(path) = args.nth(1) else {
        eprintln!(
            "Usage: render_animations <path-to-msi-file>

Will create a temporary series of files under the format `frame_%03d.jpg`.
Having such images already present will cause rendering to break. Outputs GIFs
with basenames consisting of only ASCII alphanumerics and underscores, such as
`AnimIdle9.gif` or `AnimIdleStart1_CzytanieGazety.gif`."
        );
        return Err(std::io::Error::new(std::io::ErrorKind::InvalidData, "missing argument"));
    };
    let file = File::open(&path)?;

    let extracted = chomik_extract::extract_anims(BufReader::new(file))?;
    let mut idx = 0;
    for anim in extracted {
        println!("{}", anim.filename());
        let animation = Animation::try_from(anim).unwrap();

        match animation {
            Animation::OneShot(OneShotAnimation { name, sequence: Sequence { images, .. } }) => {
                self::render_sequence(idx, name.as_ref(), images);
                idx += 1;
            }
            Animation::Looping(LoopingAnimation { name, entrance, looping, exit }) => {
                self::render_sequence(
                    idx,
                    name.as_ref(),
                    entrance.images.clone().into_iter().chain(looping.images.clone()).chain(exit.images.clone()),
                );
                idx += 1;
                self::render_sequence(idx, format!("component_{}", entrance.name).as_ref(), entrance.images);
                idx += 1;
                self::render_sequence(idx, format!("component_{}", looping.name).as_ref(), looping.images);
                idx += 1;
                self::render_sequence(idx, format!("component_{}", exit.name).as_ref(), exit.images);
                idx += 1;
            }
        }
    }

    Ok(())
}

fn render_sequence(idx: usize, name: &str, images: impl IntoIterator<Item = Image>) {
    assert!(name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'));

    let frame_files = self::write_frames(images);
    let out_fname = format!("{idx:02}_{name}.gif");
    assert!(
        Command::new("ffmpeg")
            .args(["-framerate", "60", "-i", "./frame_%04d.jpg", out_fname.as_ref()])
            .spawn()
            .unwrap()
            .wait()
            .unwrap()
            .success()
    );
    for file in frame_files {
        std::fs::remove_file(file).unwrap();
    }
}

fn write_frames(images: impl IntoIterator<Item = Image>) -> Vec<String> {
    let mut frame_files = Vec::new();
    for (fname, data) in
        images.into_iter().enumerate().map(|(idx, Image { color, .. })| (format!("frame_{idx:04}.jpg"), color.data))
    {
        println!("Writing '{fname}'...");
        let mut file = BufWriter::new(File::create(&fname).unwrap());
        file.write_all(data.as_ref()).unwrap();
        frame_files.push(fname);
    }

    frame_files
}
