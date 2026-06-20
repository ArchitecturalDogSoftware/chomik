use std::io::{Read, Seek};

use thiserror::Error;

/// A convenience macro to read data from an <code>impl [Read]</code>.
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

/// The range of support Qt resource file format versions.
///
/// Currently, only version one is supported, but there are more.
pub const SUPPORTED_VERSIONS: std::ops::Range<u32> = 1 .. 2;

/// Detects whether the given value is a magic number used by zlib.
#[must_use]
const fn is_zlib(magic: u16) -> bool {
    /// Every magic number listed as associated with zlib by <https://en.wikipedia.org/wiki/List_of_file_signatures>.
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
    /// Parse bytes into a [`Self`], leaving the input at the following byte or where the error occurred.
    ///
    /// # Errors
    ///
    /// Will return an error if the input cannot be parsed into a [`Self`] or if an error occurs whilst trying to read
    /// data.
    fn parse(input: &mut (impl Seek + Read)) -> Result<Self, ParseError>;

    /// Parse bytes into a series of bytes, until the parsing of a [`Self`] leaves the input at or after
    /// `ending_addr_exclusive`.
    ///
    /// This does not actually guarantee that `input` won't advance past `ending_addr_exclusive`, it only guarantees
    /// that another value won't be parsed after that point.
    ///
    /// # Errors
    ///
    /// Will return an error if the input cannot be parsed into a value or if an error occurs whilst trying to read
    /// data.
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

/// The opening bytes of a Qt resource file, identifying it and providing metadata.
#[derive(Debug)]
pub struct Header {
    /// The magic number at the start of the file. Must be equal to [`Self::MAGIC`].
    pub magic: [u8; 4],
    /// The Qt resource file format version.
    ///
    /// This only parses [`SUPPORTED_VERSIONS`].
    pub version: u32,
    /// The byte address in the file where the [tree][`QrcFile::tree`] begins.
    ///
    /// This is not a trusted value.
    pub tree_addr: u32,
    /// The byte address in the file where the [files][`QrcFile::files`] begin.
    ///
    /// This is not a trusted value.
    pub files_addr: u32,
    /// The byte address in the file where the [filenames][`QrcFile::names`] begin.
    ///
    /// This is not a trusted value.
    pub names_addr: u32,
}

impl Header {
    /// The magic number that identifies a Qt resource file.
    pub const MAGIC: [u8; 4] = *b"qres";
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

/// A file embedded with a Qt resource file.
///
/// May or may not be compressed.
#[derive(Debug)]
pub struct File {
    // TO-DO: is this actually a trusted value?
    /// The length of the file in bytes. This includes [`Self::decompressed_len`] if present, so [`Self::data`] will be
    /// [`size_of::<u32>()`][`size_of`] bytes smaller if [`Self`] is compressed.
    pub len: u32,
    /// If [`Self::data`] is compressed, this is the reported length of the decompressed data.
    ///
    /// This is not a trusted value.
    pub decompressed_len: Option<u32>,
    /// The actual data of a file. Should be of length [`Self::len`] if [`Self::decompressed_len`] is [`None`], or
    /// <code>[Self::len] - [size_of::<u32>()][`size_of`]</code> if it is [`Some`].
    pub data: Box<[u8]>,
}

impl Parse for File {
    fn parse(input: &mut (impl Seek + Read)) -> Result<Self, ParseError> {
        let len = read!(input, u32);

        #[expect(clippy::cast_possible_wrap, reason = "it won't")]
        let decompressed_len = {
            input.seek_relative(size_of::<u32>() as i64)?;
            let maybe_zlib_magic = read!(input, u16);
            input.seek_relative(-((size_of::<u32>() + size_of::<u16>()) as i64))?;

            if self::is_zlib(maybe_zlib_magic) { Some(read!(input, u32)) } else { None }
        };

        // `len` includes the `decompressed_len` field, so if that's present then the data buffer is slightly smaller.
        let actual_data_len = len as usize - if decompressed_len.is_some() { size_of::<u32>() } else { 0 };
        let data = read!(input, [u8; actual_data_len]);

        Ok(Self { len, decompressed_len, data })
    }
}

#[derive(Debug)]
pub struct Filename {
    /// The number of [`u16`]s in [`Self::name`]. Does not include [`Self::hash`].
    pub len: u16,
    /// A hash of something, presumably of [`Self::name`] or the file.
    ///
    /// I have not investigated this at all.
    pub hash: u32,
    /// The UTF-16 bytes of the filename. Should be of length [`Self::len`].
    pub name: Box<[u16]>,
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

/// The data held by a [`Node`], representing either a directory or a file.
#[derive(Debug)]
pub enum NodeData {
    /// A directory in this Qt resource file's filesystem.
    Directory {
        /// The number of children in this directory.
        ///
        /// This is not a trusted value.
        child_count: u32,
        // TO-DO: this is very likely an offset as well.
        /// The index of [`QrcFile::names`] where the first child can be found.
        ///
        /// This is not a trusted value.
        first_child_idx: u32,
    },
    /// A file in this Qt resource file's filesystem.
    File {
        /// A number identifying a country, presumably for localization purposes.
        ///
        /// I have not investigated this at all.
        country: u16,
        /// A number identifying a language, presumably for localization purposes.
        ///
        /// I have not investigated this at all.
        language: u16,
        /// The offset in bytes from the start of [`QrcFile::files`] where the first child can be found.
        ///
        /// This is not a trusted value.
        files_offset: u32,
    },
}

/// A node in a Qt resource file's filesystem tree.
#[derive(Debug)]
pub struct Node {
    /// The offset in bytes from the start of [`QrcFile::names`] where this file's name can be found.
    ///
    /// This is not a trusted value.
    pub names_offset: u32,
    /// A flag that indicates the type of [`NodeData`] this is.
    ///
    /// Can be one of: [`Self::FLAG_NONE`], [`Self::FLAG_COMPRESSED`], [`Self::FLAG_DIRECTORY`], or
    /// [`Self::FLAG_COMPRESSED_ZSTD`]. This field also may also be bit flags, in which case it may be combination of
    /// those value. I am not yet sure whether these are discrete values or bit flags.
    pub flag: u16,
    /// The actual data held by this node.
    pub data: NodeData,
}

impl Parse for Node {
    fn parse(input: &mut (impl Seek + Read)) -> Result<Self, ParseError> {
        let names_offset = read!(input, u32);
        let flag = read!(input, u16);
        let data = if flag == Self::FLAG_DIRECTORY {
            NodeData::Directory { child_count: read!(input, u32), first_child_idx: read!(input, u32) }
        } else {
            NodeData::File { country: read!(input, u16), language: read!(input, u16), files_offset: read!(input, u32) }
        };

        Ok(Self { names_offset, flag, data })
    }
}

impl Node {
    /// Indicates that the file is compressed with zlib.
    const FLAG_COMPRESSED: u16 = 1;
    /// Indicates that the file is compressed with Zstandard.
    const FLAG_COMPRESSED_ZSTD: u16 = 4;
    /// Indicates that this is a directory node, not file.
    const FLAG_DIRECTORY: u16 = 2;
    // TO-DO: is this true?
    /// Indicates that this is an uncompressed file.
    const FLAG_NONE: u16 = 0;

    #[must_use]
    pub const fn is_none(&self) -> bool {
        self.flag == Self::FLAG_NONE
    }

    #[must_use]
    pub const fn is_compressed(&self) -> bool {
        self.flag == Self::FLAG_COMPRESSED
    }

    #[must_use]
    pub const fn is_directory(&self) -> bool {
        self.flag == Self::FLAG_DIRECTORY
    }

    #[must_use]
    pub const fn is_compressed_zstd(&self) -> bool {
        self.flag == Self::FLAG_COMPRESSED_ZSTD
    }
}

/// Holds some data alongside the byte address within the file where it was original located.
#[derive(Debug)]
pub struct Located<T: ?Sized> {
    /// The byte address within the file where [`Self::data`] original started.
    pub original_addr: u32,
    /// The data parsed, starting from [`Self::original_addr`].
    pub data: T,
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

/// A Qt resource file.
#[derive(Debug)]
pub struct QrcFile {
    pub header: Header,
    /// The actual data of the contained files.
    ///
    /// Arranged in ascending order of [`Located::original_addr`].
    pub files: Box<[Located<File>]>,
    /// The names of the contained files.
    ///
    /// Arranged in ascending order of [`Located::original_addr`].
    pub names: Box<[Located<Filename>]>,
    /// The nodes which represent each contained file, ready to be parsed into a directory tree.
    ///
    /// Arranged in ascending order of [`Located::original_addr`].
    pub tree: Box<[Located<Node>]>,
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
