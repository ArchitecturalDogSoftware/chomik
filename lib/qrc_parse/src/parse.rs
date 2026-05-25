use std::io::{Read, Seek};

use thiserror::Error;

macro_rules! read {
    // Specializes on byte slices for performance.
    ($input:expr,[u8; $count:expr]) => {{
        let mut buf = vec![0_u8; ($count).into()];
        ($input).read_exact(buf.as_mut_slice())?;
        buf.into_boxed_slice()
    }};
    // Provides a convenient API for reading slices of fixed length that are only known at runtime.
    ($input:expr,[$ty:ty; $count:expr]) => {{
        let mut buf = Vec::with_capacity(($count).into());
        for _ in 0 .. ($count) {
            buf.push(read!($input, $ty));
        }
        buf.into_boxed_slice()
    }};
    // Provides a convenient API for reading anything that provides `from_be_bytes`.
    ($input:expr, $ty:ty) => {
        <$ty>::from_be_bytes(($input).read_array()?)
    };
}

pub const SUPPORTED_VERSIONS: std::ops::Range<u32> = 1 .. 2;

const fn is_zlib(magic: u16) -> bool {
    const ZLIB_MAGIC_NUMBERS: [u16; 8] = [0x78_01, 0x78_5E, 0x78_9C, 0x78_DA, 0x78_20, 0x78_7D, 0x78_BB, 0x78_F9];

    let mut i = 0;
    while i < ZLIB_MAGIC_NUMBERS.len() {
        if magic == ZLIB_MAGIC_NUMBERS[i] {
            return true;
        }

        i += 1;
    }

    false
}

pub trait Parse: Sized {
    fn parse(input: &mut (impl Seek + Read)) -> Result<Self, ParseError>;

    fn parse_until(input: &mut (impl Seek + Read), ending_addr_exclusive: u32) -> Result<Box<[Self]>, ParseError> {
        let mut out = Vec::new();

        while input.stream_position()? < ending_addr_exclusive.into() {
            out.push(Self::parse(input)?);
        }

        Ok(out.into_boxed_slice())
    }
}

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ParseError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    // Should this just be the invalid data I/O error?
    #[error("opening bytes did not match the expected magic number")]
    MagicMismatch,
    /// The Qt resource file format version was not contained within [`SUPPORTED_VERSIONS`];
    #[error("unsupported Qt resource file format version")]
    UnsupportedVersion,
}

#[derive(Debug)]
struct Header {
    magic: [u8; 4],
    version: u32,
    tree_addr: u32,
    files_addr: u32,
    names_addr: u32,
}

impl Header {
    const MAGIC: [u8; 4] = *b"qres";
}

impl Parse for Header {
    fn parse(input: &mut (impl Seek + Read)) -> Result<Self, ParseError> {
        Ok(Self {
            magic: {
                let magic = input.read_array::<{ Self::MAGIC.len() }>()?;

                if magic != Self::MAGIC {
                    return Err(ParseError::MagicMismatch);
                }

                magic
            },
            version: {
                let version = read!(input, u32);

                if !SUPPORTED_VERSIONS.contains(&version) {
                    return Err(ParseError::UnsupportedVersion);
                }

                version
            },
            tree_addr: read!(input, u32),
            files_addr: read!(input, u32),
            names_addr: read!(input, u32),
        })
    }
}

#[derive(Debug)]
struct File {
    len: u32,
    /// If [`Self::data`] is compressed, this is the reported length of the decompressed data.
    ///
    /// This is not a trusted value.
    decompressed_len: Option<u32>,
    /// The actual data of a file. Should be of length [`Self::len`] if [`Self::decompressed_len`]
    /// is [`None`], or <code>[Self::len] - 4</code> if it is [`Some`].
    data: Box<[u8]>,
}

impl Parse for File {
    fn parse(input: &mut (impl Seek + Read)) -> Result<Self, ParseError> {
        let len = read!(input, u32);

        #[expect(clippy::cast_possible_wrap, reason = "it won't")]
        let decompressed_len = {
            input.seek_relative(size_of::<u32>() as i64)?;
            let maybe_zlib_magic = read!(input, u16);
            input.seek_relative(-((size_of::<u32>() + size_of::<u16>()) as i64))?;

            if is_zlib(maybe_zlib_magic) { Some(read!(input, u32)) } else { None }
        };

        // `len` includes the `decompressed_len` field, so if that's present then the data
        // buffer is slightly smaller.
        let actual_data_len = len as usize - if decompressed_len.is_some() { size_of::<u32>() } else { 0 };
        let data = read!(input, [u8; actual_data_len]);

        Ok(Self { len, decompressed_len, data })
    }
}

#[derive(Debug)]
struct Filename {
    len: u16,
    hash: u32,
    /// The UTF-16 bytes of the filename. Should be of length [`Self::len`].
    name: Box<[u16]>,
}

impl Parse for Filename {
    fn parse(input: &mut (impl Seek + Read)) -> Result<Self, ParseError> {
        let len = read!(input, u16);

        Ok(Self {
            len, //
            hash: read!(input, u32),
            name: read!(input, [u16; len]),
        })
    }
}

#[derive(Debug)]
enum NodeData {
    Directory { child_count: u32, child_offset: u32 },
    File { country: u16, language: u16, files_offset: u32 },
}

#[derive(Debug)]
struct Node {
    names_offset: u32,
    // TO-DO: is this a bitmask or an enum?
    flag: u16,
    data: NodeData,
}

impl Parse for Node {
    fn parse(input: &mut (impl Seek + Read)) -> Result<Self, ParseError> {
        let names_offset = read!(input, u32);
        let flag = read!(input, u16);
        let data = if flag == Self::FLAG_DIRECTORY {
            NodeData::Directory { child_count: read!(input, u32), child_offset: read!(input, u32) }
        } else {
            NodeData::File { country: read!(input, u16), language: read!(input, u16), files_offset: read!(input, u32) }
        };

        Ok(Self { names_offset, flag, data })
    }
}

impl Node {
    const FLAG_COMPRESSED: u16 = 1;
    const FLAG_COMPRESSED_ZSTD: u16 = 4;
    const FLAG_DIRECTORY: u16 = 2;
    const FLAG_NONE: u16 = 0;

    const fn is_none(&self) -> bool {
        self.flag == Self::FLAG_NONE
    }

    const fn is_compressed(&self) -> bool {
        self.flag == Self::FLAG_COMPRESSED
    }

    const fn is_directory(&self) -> bool {
        self.flag == Self::FLAG_DIRECTORY
    }

    const fn is_compressed_zstd(&self) -> bool {
        self.flag == Self::FLAG_COMPRESSED_ZSTD
    }
}

#[derive(Debug)]
struct Located<T: ?Sized> {
    original_addr: u32,
    data: T,
}

impl<T: Parse> Parse for Located<T> {
    fn parse(input: &mut (impl Seek + Read)) -> Result<Self, ParseError> {
        Ok(Self {
            original_addr: input
                .stream_position()?
                .try_into()
                .map_err(|_| std::io::Error::from(std::io::ErrorKind::FileTooLarge))?,
            data: T::parse(input)?,
        })
    }
}

#[derive(Debug)]
pub struct QrcFile {
    header: Header,
    files: Box<[Located<File>]>,
    names: Box<[Located<Filename>]>,
    tree: Box<[Located<Node>]>,
}

impl Parse for QrcFile {
    fn parse(input: &mut (impl Seek + Read)) -> Result<Self, ParseError> {
        input.rewind()?;
        let len = input.stream_len()?.try_into().map_err(|_| std::io::Error::from(std::io::ErrorKind::FileTooLarge))?;

        let header = Header::parse(input)?;
        Ok(Self {
            files: Located::<File>::parse_until(input, header.names_addr)?,
            names: Located::<Filename>::parse_until(input, header.tree_addr)?,
            tree: Located::<Node>::parse_until(input, len)?,
            header,
        })
    }
}
