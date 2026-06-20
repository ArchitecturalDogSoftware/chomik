#![feature(read_array)]
#![feature(seek_stream_len)] // Should I?

use std::char;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Result, Seek};

use crate::parse::Parse;

mod parse;

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
enum NodeData {
    Directory { children: Box<[Node]> },
    File { data: Box<[u8]> },
}

#[derive(Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
struct Node {
    name: Box<str>,
    data: NodeData,
}

impl Node {
    fn convert_node(from: &parse::Node, parsed: &parse::QrcFile) -> Result<Self> {
        let name = self::get_name(parsed, from.names_offset)?;
        let data = match from.data {
            parse::NodeData::Directory { child_count, first_child_idx } => {
                NodeData::Directory { children: Box::new([]) }
            }
            parse::NodeData::File { country, language, files_offset } => NodeData::File {
                data: self::get_by_offset(&parsed.files, files_offset)
                    .ok_or_else(|| {
                        std::io::Error::new(
                            std::io::ErrorKind::InvalidData,
                            "no file at QRC file tree node's file offset",
                        )
                    })?
                    .data
                    .clone(),
            },
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
            NodeData::File { data } => {
                out.push(File { name: self.name.as_ref(), dir: directory_stack.join("/").into_boxed_str(), data });
            }
        }
    }

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
    data: &'n [u8],
}

impl<'n> File<'n> {
    pub fn name(&self) -> &'n str {
        self.name
    }

    pub fn dir(&self) -> &str {
        &self.dir
    }

    pub fn path(&self) -> Box<str> {
        format!("{}/{}", self.dir, self.name).into_boxed_str()
    }

    pub fn data(&self) -> &[u8] {
        self.data
    }
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

fn get_by_offset<T>(arr: &[parse::Located<T>], offset: u32) -> Option<&T> {
    let addr = offset + arr.first()?.original_addr;
    let idx = arr.binary_search_by_key(&addr, |v| v.original_addr).ok()?;
    Some(&arr.get(idx).expect("`binary_search_by_key` should always return a valid index").data)
}
