use godot::global::{godot_error, godot_warn};
use nom::{self, IResult};
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
};

use crate::unwrap_or_return;

const PARSE_CHUNK_SIZE: usize = 1024 * 1024 * 10;
const MAX_PARSE_CHUNK_SIZE: usize = 1024 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Default)]
enum ParserState {
    #[default]
    SceneHeader,
    ObjectHeader,
    ObjectValue,
}

pub fn check_tscn(absolute_offset: u64, size: u64, pck: &mut BufReader<File>) -> bool {
    let mut buffer = vec![0u8; PARSE_CHUNK_SIZE].into_boxed_slice();
    let mut parser = TSCNParser::default();

    unwrap_or_return!(pck.seek(SeekFrom::Start(absolute_offset)), false, Result);
    let mut pck = pck.take(size);

    loop {
        let Ok(bytes_read) = pck.read(&mut buffer) else {
            godot_error!("error while reading file");
            return false;
        };
        if bytes_read == 0 {
            if !parser.parsing_completed() {
                godot_error!("reached end of file but failed to parse final value");
                return false;
            }
            return true;
        }

        parser.add_to_buffer(&buffer[..bytes_read]);

        if !parser.parse_buffer() {
            godot_warn!("scene failed to parse!");
            return false;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Default)]
struct TSCNParser {
    state: ParserState,
    buffer: Vec<u8>,
}

impl TSCNParser {
    fn add_to_buffer(&mut self, addition: &[u8]) {
        self.buffer.extend(addition);
    }
    fn parse_buffer(&mut self) -> bool {
        let mut slice = &*self.buffer;
        loop {
            let result = match self.state {
                ParserState::SceneHeader => Self::parse_scene_header(&mut slice, &mut self.state),
                ParserState::ObjectHeader => Self::parse_object_header(&mut slice, &mut self.state),
                ParserState::ObjectValue => Self::parse_object_value(&mut slice, &mut self.state),
            };
            match result {
                Some(true) => continue,      // parsed without issue
                Some(false) => return false, // failed to parse
                None => {
                    self.buffer.drain(..self.buffer.len() - slice.len());
                    return true;
                } // reached end of buffer before finishing
            }
        }
    }
    fn parse_scene_header(slice: &mut &[u8], state: &mut ParserState) -> Option<bool> {
        let Ok(str_slice) = str::from_utf8(&slice) else {
            return Some(false);
        };

        let result: IResult<&str, &str> = nom::branch::alt((
            nom::bytes::streaming::tag("[gd_scene format=4]"),
            nom::bytes::streaming::tag("[gd_scene format=3]"),
        ))(str_slice);

        match result {
            Ok((_, taken)) => {
                *state = ParserState::ObjectHeader;
                *slice = &slice[taken.len()..];
                Some(true)
            }
            Err(nom::Err::Incomplete(_)) => None,
            Err(_) => Some(false),
        }
    }
    fn parse_object_header(slice: &mut &[u8], state: &mut ParserState) -> Option<bool> {}
    fn parse_object_value(slice: &mut &[u8], state: &mut ParserState) -> Option<bool> {}
    fn parsing_completed(&mut self) -> bool {
        let result = self.buffer.is_empty();
        self.buffer.clear();
        return result;
    }
}

impl Drop for TSCNParser {
    fn drop(&mut self) {
        if !self.parsing_completed() {
            panic!("tried to drop parser but we never checked if parsing finished!")
        }
    }
}
