#![feature(read_array)]
#![feature(seek_stream_len)] // Should I?

use std::char;
use std::io::{Read, Result, Seek};

use crate::parse::Parse;

mod parse;

#[derive(Debug, Hash, PartialEq, Eq)]
enum NodeData {
    Directory { children: Box<[Node]> },
    File { compression_type: CompressionType, data: Box<[u8]> },
}

#[derive(Debug, Hash, PartialEq, Eq)]
struct Node {
    name: Box<str>,
    data: NodeData,
}

impl Node {
    fn convert_node(from: &parse::Node, parsed: &parse::QrcFile) -> Result<Self> {
        let name = self::get_name(parsed, from.names_offset)?;
        let data = match from.data {
            parse::NodeData::Directory { .. } => NodeData::Directory { children: Box::new([]) },
            parse::NodeData::File { files_offset, .. } => {
                let file = self::get_by_offset(&parsed.files, files_offset).ok_or_else(|| {
                    std::io::Error::new(std::io::ErrorKind::InvalidData, "no file at QRC file tree node's file offset")
                })?;
                let compression_type = if !from.is_compressed() {
                    CompressionType::None
                } else if from.is_compressed_zlib() {
                    CompressionType::Zlib {
                        decompressed_len: file
                            .decompressed_len
                            .expect("a compressed file should always have a `decompressed_len`"),
                    }
                } else {
                    unreachable!("Qt resource files of format version 1 can only be compressed with zlib")
                };

                NodeData::File { compression_type, data: file.data.clone() }
            }
        };

        Ok(Self { name, data })
    }

    fn files_impl<'n>(&'n self, directory_stack: &mut Vec<&'n str>, out: &mut Vec<File<'n>>) {
        match &self.data {
            NodeData::Directory { children } => {
                directory_stack.push(self.name.as_ref());
                for file in children {
                    file.files_impl(directory_stack, out);
                }
                directory_stack.pop();
            }
            NodeData::File { compression_type, data } => {
                out.push(File {
                    name: self.name.as_ref(),
                    dir: directory_stack.join("/").into_boxed_str(),
                    compression_type: *compression_type,
                    data,
                });
            }
        }
    }

    #[must_use]
    pub fn files(&self) -> Box<[File<'_>]> {
        let mut out = Vec::new();
        // Make sure that `.join("/")` actually adds a leading slash.
        let mut directory_stack = vec![""];

        self.files_impl(&mut directory_stack, &mut out);

        out.into_boxed_slice()
    }
}

pub struct File<'n> {
    name: &'n str,
    dir: Box<str>,
    compression_type: CompressionType,
    data: &'n [u8],
}

impl<'n> File<'n> {
    #[must_use]
    pub const fn name(&self) -> &'n str {
        self.name
    }

    #[must_use]
    pub fn dir(&self) -> &str {
        &self.dir
    }

    #[must_use]
    pub fn path(&self) -> Box<str> {
        format!("{}/{}", self.dir, self.name).into_boxed_str()
    }

    /// Returns the raw bytes stored in the Qt resource file. If [`Self::compression_type()`] is
    /// [`CompressionType::None`], this is the actual data of the file. Otherwise, it's the compressed data.
    #[must_use]
    pub const fn raw_data(&self) -> &[u8] {
        self.data
    }

    #[must_use]
    pub const fn compression_type(&self) -> CompressionType {
        self.compression_type
    }

    #[must_use]
    pub fn decompressed_data(&self) -> std::result::Result<Box<[u8]>, DecompressionError> {
        match self.compression_type {
            CompressionType::None => Ok(self.data.into()),
            CompressionType::Zlib { decompressed_len } => {
                let mut buf = vec![0_u8; decompressed_len as usize];

                let (decompressed, rc) =
                    zlib_rs::decompress_slice(&mut buf, self.data, zlib_rs::InflateConfig::default());

                match rc {
                    zlib_rs::ReturnCode::Ok => (),
                    zlib_rs::ReturnCode::BufError => return Err(DecompressionError::BadDecompressedLength),
                    _ => return Err(DecompressionError::Zlib(rc)),
                }

                // `decompressed` is a subslice of `buf`. As long as we sanity check that it begins at the same
                // place as `buf`, truncating `buf` to be the same length should mean that they
                // become equal.
                let start_addr = decompressed.as_ptr();
                let len = decompressed.len();
                if start_addr != buf.as_ptr() {
                    return Err(DecompressionError::BadOutput);
                }
                buf.truncate(len);

                Ok(buf.into_boxed_slice())
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum DecompressionError {
    #[error("decompression returned unexpected subslice")]
    BadOutput,
    #[error("QRC file provided an inaccurate length of decompressed data")]
    BadDecompressedLength,
    #[error("decompression failed with code {0:?}")]
    Zlib(zlib_rs::ReturnCode),
}

#[derive(Clone, Copy, Debug, Hash, PartialEq, Eq)]
pub enum CompressionType {
    None,
    Zlib { decompressed_len: u32 },
}

#[derive(Debug, Hash, PartialEq, Eq)]
pub struct AnimFile {
    /// The children of the root node.
    top_level: Box<[Node]>,
}

impl AnimFile {
    pub fn parse<R: Seek + Read>(mut reader: R) -> Result<Self> {
        let parsed = parse::QrcFile::parse(&mut reader).map_err(|e| match e {
            parse::ParseError::Io(error) => error,
            _ => std::io::Error::new(std::io::ErrorKind::InvalidData, e.to_string()),
        })?;

        let top_level = if let Some(first) = parsed.tree.first() {
            let NodeData::Directory { children } = self::visit(first, &parsed)?.data else {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "root node in file tree is a file (expected a directory)",
                ));
            };

            children
        } else {
            Box::new([])
        };

        Ok(Self { top_level })
    }

    #[must_use]
    pub fn files(&self) -> Box<[File<'_>]> {
        self.top_level.iter().flat_map(Node::files).collect()
    }
}

fn visit(node: &parse::Located<parse::Node>, parsed: &parse::QrcFile) -> Result<Node> {
    let mut new_node = Node::convert_node(&node.data, parsed)?;

    if let parse::NodeData::Directory { child_count, first_child_idx } = &node.data.data {
        let children: Box<[Node]> = parsed.tree[*first_child_idx as usize ..][.. *child_count as usize]
            .iter()
            .map(|n| self::visit(n, parsed))
            .collect::<Result<_>>()?;

        new_node.data = NodeData::Directory { children };
    }

    Ok(new_node)
}

fn get_name(parsed: &parse::QrcFile, names_offset: u32) -> Result<Box<str>> {
    let name: &parse::Filename = self::get_by_offset(&parsed.names, names_offset).ok_or_else(|| {
        std::io::Error::new(std::io::ErrorKind::InvalidData, "no filename at QRC file tree node's filename offset")
    })?;

    char::decode_utf16(name.name.iter().copied()) //
        .collect::<std::result::Result<_, char::DecodeUtf16Error>>()
        .map_err(|_| {
            std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("invalid UTF-16 codepoint at name index {names_offset}"),
            )
        })
}

#[must_use]
fn get_by_offset<T>(arr: &[parse::Located<T>], offset: u32) -> Option<&T> {
    let addr = offset + arr.first()?.original_addr;
    let idx = arr.binary_search_by_key(&addr, |v| v.original_addr).ok()?;
    Some(&arr.get(idx).expect("`binary_search_by_key` should always return a valid index").data)
}
