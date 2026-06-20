use std::io::{Read, Result, Seek};
use std::num::NonZero;

use cab::Cabinet;
use msi::{Expr, Package, Select};

#[must_use]
fn invalid_data(reason: &str) -> std::io::Error {
    std::io::Error::new(std::io::ErrorKind::InvalidData, reason)
}

fn extract_non_zero_u32(row: &msi::Row, key: &str) -> Result<NonZero<u32>> {
    row[key]
        .as_int()
        .ok_or_else(|| self::invalid_data(format!("`{key}` should be an integer").as_str()))?
        .try_into()
        .ok()
        .and_then(NonZero::new)
        .ok_or_else(|| self::invalid_data(format!("`{key}` should be >=1").as_str()))
}

#[derive(Clone)]
struct Filename {
    short_name: String,
    long_name: Option<String>,
}

impl Filename {
    #[must_use]
    fn name(&self) -> &str {
        self.long_name.as_deref().unwrap_or(self.short_name.as_ref())
    }
}

// Some of these could be smaller types, but whatever.
pub struct AnimFile {
    id: String,
    name: Filename,
    size: u32,
    sequence_no: NonZero<u32>,
}

impl AnimFile {
    fn list(package: &mut Package<impl Read + Seek>) -> Result<Box<[Self]>> {
        // Tables and columns in play here:
        //
        // - [File](https://learn.microsoft.com/en-us/windows/win32/msi/file-table)
        //   - `File`
        //   - `Component_`
        //   - `FileName`
        //   - `FileSize`
        // - [Component](https://learn.microsoft.com/en-us/windows/win32/msi/component-table)
        //   - `Component`
        //   - `Directory_`
        //
        // You could pretty easily make this path-aware by inner joining on the [Directory
        // table](https://learn.microsoft.com/en-us/windows/win32/msi/directory-table) and working up the directory tree.
        package
            .select_rows(
                Select::table("File")
                    .inner_join(
                        Select::table("Component"),
                        Expr::col("File.Component_").eq(Expr::col("Component.Component")),
                    )
                    .with(Expr::col("Component.Directory_").eq(Expr::string("ANIMDIR")))
                    .columns(&["File.File", "File.FileName", "File.FileSize", "File.Sequence"]),
            )?
            .map(|row| {
                Ok(Self {
                    id: row["File.File"]
                        .as_str()
                        .ok_or_else(|| self::invalid_data("`File.FileName` should be a string identifier"))?
                        .to_string(),
                    name: {
                        let str = row["File.FileName"]
                            .as_str()
                            .ok_or_else(|| self::invalid_data("`File.FileName` should be a string"))?;
                        let (short, long) = str //
                            .split_once('|')
                            .map_or((str, None), |(short, long)| (short, Some(long)));

                        Filename { short_name: short.to_string(), long_name: long.map(str::to_string) }
                    },
                    size: row["File.FileSize"]
                        .as_int()
                        .ok_or_else(|| self::invalid_data("`File.FileSize` should be an integer"))?
                        .try_into()
                        .map_err(|_| self::invalid_data("`File.FileSize` should be non-negative"))?,
                    sequence_no: self::extract_non_zero_u32(&row, "File.Sequence")?,
                })
            })
            .collect()
    }

    #[must_use]
    pub fn filename(&self) -> &str {
        self.name.name()
    }
}

struct EmbeddedCabinet {
    id: String,
    last_sequence_no: NonZero<u32>,
}

impl EmbeddedCabinet {
    fn list(package: &mut Package<impl Read + Seek>) -> Result<Box<[Self]>> {
        package
        .select_rows(Select::table("Media").columns(&["LastSequence", "Cabinet"]))?
        .map(|row| {
            let cabinet_name = row["Cabinet"]
                .as_str()
                .ok_or_else(|| self::invalid_data("`Media.Cabinet` should be a string identifier"))?;

            Ok(Self {
                id: if cabinet_name.starts_with('#') { cabinet_name.split_at(1).1 } else { "" }.to_string(),
                last_sequence_no: self::extract_non_zero_u32(&row, "LastSequence")?,
            })
        })
        // An empty identifier at this point means that it's either not embedded or just weird. 
        .filter(|maybe_cabinet| !matches!(maybe_cabinet, Ok(c) if c.id.is_empty()))
        .collect()
    }
}

pub struct File<'r, R: Read + Seek> {
    name: Filename,
    reader: cab::FileReader<'r, msi::StreamReader<R>>,
}

impl<'r, R: Read + Seek> File<'r, R> {
    #[must_use]
    pub fn filename(&self) -> &str {
        self.name.name()
    }

    #[must_use]
    pub fn into_reader(self) -> impl 'r + Seek + Read {
        self.reader
    }

    #[must_use]
    pub fn as_reader(&mut self) -> &mut (impl 'r + Seek + Read) {
        &mut self.reader
    }
}

pub struct Cabinets<R: Read + Seek> {
    /// Must be kept around, otherwise a [`std::rc::Weak`] allows the data backing [`Self::cabinets`] to be dropped.
    _package: Package<R>,
    cabinets: Box<[(EmbeddedCabinet, Cabinet<msi::StreamReader<R>>)]>,
}

impl<R: Read + Seek> Cabinets<R> {
    fn new(mut package: Package<R>, cabinets: Box<[EmbeddedCabinet]>) -> Result<Self> {
        Ok(Self {
            cabinets: cabinets
                .into_iter()
                .map(|cabinet| {
                    let parsed = Cabinet::new(package.read_stream(cabinet.id.as_str())?)?;
                    Ok((cabinet, parsed))
                })
                .collect::<Result<_>>()?,
            _package: package,
        })
    }

    pub fn get_file<'r>(&'r mut self, file: &AnimFile) -> Result<File<'r, R>> {
        let (_, cabinet) =
            self.cabinets.iter_mut().find(|(cabinet, _)| file.sequence_no <= cabinet.last_sequence_no).ok_or_else(
                || std::io::Error::new(std::io::ErrorKind::NotFound, "file not found in any embedded cabinet the MSI"),
            )?;

        // Short or long? Both?
        Ok(File { name: file.name.clone(), reader: cabinet.read_file(file.name.name())? })
    }
}

pub fn extract_anims<R: Read + Seek>(msi: R) -> Result<(Box<[AnimFile]>, Cabinets<R>)> {
    let mut package = Package::open(msi)?;
    let files = AnimFile::list(&mut package)?;
    let embedded_cabinets = EmbeddedCabinet::list(&mut package)?;
    let cabinets = Cabinets::new(package, embedded_cabinets)?;

    Ok((files, cabinets))
}
