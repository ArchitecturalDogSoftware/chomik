#![feature(read_array)]
#![feature(seek_stream_len)] // Should I?

use std::io::{Read, Seek};

pub mod parse;

pub struct AnimFile;

impl AnimFile {
    pub fn parse<R: Seek + Read>(reader: R) -> std::io::Result<Self> {
        todo!()
    }
}
