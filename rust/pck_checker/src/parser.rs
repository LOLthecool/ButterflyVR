use godot::global::{godot_error, godot_warn};
use nom::bytes::streaming;
use nom::{self, IResult, character};
use nom::{Parser, sequence};
use nom::{branch, combinator};
use std::cell::Cell;
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
};
use strum::IntoEnumIterator;

use crate::{resources, unwrap_or_return};

const PARSE_CHUNK_SIZE: usize = 1024 * 1024 * 50;
const MAX_PARSE_CHUNK_SIZE: usize = 1024 * 1024 * 1000;

macro_rules! handle_incomplete {
    ($result:expr) => {
        match $result {
            Ok(x) => x,
            Err(nom::Err::Incomplete(_)) => return None,
            Err(_) => return Some(false),
        }
    };
}

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
    extend_failed: bool,
}

impl TSCNParser {
    fn add_to_buffer(&mut self, addition: &[u8]) {
        if self.buffer.len() + addition.len() > MAX_PARSE_CHUNK_SIZE {
            self.extend_failed = true;
            return;
        }
        self.buffer.extend(addition);
    }
    fn parse_buffer(&mut self) -> bool {
        if self.extend_failed {
            return false;
        }

        let mut slice = &*self.buffer;
        loop {
            let result = match self.state {
                ParserState::SceneHeader => Self::parse_scene_header(&mut slice, &mut self.state),
                ParserState::ObjectHeader => Self::parse_object_header(&mut slice, &mut self.state),
                ParserState::ObjectValue => {
                    Self::parse_object_key_value(&mut slice, &mut self.state)
                }
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

        let result: IResult<&str, &str> = branch::alt((
            streaming::tag("[gd_scene format=4]"),
            streaming::tag("[gd_scene format=3]"),
        ))(str_slice);

        let (str_slice, _) = handle_incomplete!(result);

        *state = ParserState::ObjectHeader;
        *slice = &slice[slice.len() - str_slice.as_bytes().len()..];
        Some(true)
    }
    fn parse_object_header(slice: &mut &[u8], state: &mut ParserState) -> Option<bool> {
        let Ok(str_slice) = str::from_utf8(&slice) else {
            return Some(false);
        };

        let (str_slice, _) = handle_incomplete!(character::streaming::multispace0::<
            &str,
            nom::error::Error<&str>,
        >(str_slice));

        let (str_slice, object_type) = handle_incomplete!(branch::alt((
            streaming::tag::<&str, &str, nom::error::Error<_>>("[sub_resource"),
            streaming::tag("[node"),
        ))(str_slice));

        loop {
            let mut resource_type = String::new();

            let (str_slice, taken) = handle_incomplete!(branch::alt((
                |str_slice| Self::parse_header_kv(str_slice, &mut resource_type),
                streaming::tag("]"),
            ))(str_slice));

            *slice = &slice[slice.len() - str_slice.as_bytes().len()..];

            if taken == "]" {
                *state = ParserState::ObjectValue;
                return Some(true);
            }

            if !resource_type.is_empty() && object_type == "[sub_resource" {
                if !resources::ResourceWhitelist::iter()
                    .any(|x| resource_type == x.to_string().as_str())
                {
                    return Some(false);
                }
            }

            continue;
        }
    }
    fn parse_object_key_value(slice: &mut &[u8], state: &mut ParserState) -> Option<bool> {
        let Ok(str_slice) = str::from_utf8(&slice) else {
            return Some(false);
        };

        let (str_slice, _) = handle_incomplete!(character::streaming::multispace0::<
            &str,
            nom::error::Error<&str>,
        >(str_slice));

        match streaming::tag::<&str, &str, nom::error::Error<&str>>("[")(str_slice) {
            Ok((str_slice, _)) => {
                *state = ParserState::ObjectHeader;
                // dont update str_slice length since header parsing expects leading [
                *slice = &slice[slice.len() - str_slice.as_bytes().len()..];
                return Some(true);
            }
            Err(nom::Err::Incomplete(_)) => {
                return None;
            }
            Err(_) => {}
        };

        let (str_slice, _) = handle_incomplete!(sequence::terminated(
            nom::multi::many1_count(branch::alt((
                character::streaming::alpha1::<&str, nom::error::Error<&str>>,
                character::streaming::char('_').map(|_| "")
            ))),
            sequence::delimited(
                character::streaming::multispace0,
                streaming::tag("="),
                character::streaming::multispace0,
            ),
        )(str_slice));

        let (str_slice, _) = handle_incomplete!(sequence::terminated(
            Self::parse_value,
            character::streaming::multispace1 // always at least 1 seperating newline
        )(str_slice));

        *slice = &slice[slice.len() - str_slice.as_bytes().len()..];
        return Some(true);
    }

    fn parse_value(slice: &str) -> IResult<&str, &str> {
        branch::alt((
            Self::parse_string,
            Self::parse_container,
            Self::parse_number_literal,
            Self::parse_identifier,
        ))(slice)
    }

    fn parse_string(slice: &str) -> IResult<&str, &str> {
        let last = Cell::new(' ');
        sequence::delimited(
            sequence::tuple((
                combinator::opt(character::streaming::one_of("&^")),
                streaming::tag("\""),
            )),
            streaming::take_till(|x: char| {
                if x == '"' && last.get() != '\\' {
                    last.set(' ');
                    return true;
                }
                last.set(x);
                false
            }),
            streaming::tag("\""),
        )(slice)
    }

    fn parse_container(slice: &str) -> IResult<&str, &str> {}

    fn parse_number_literal(slice: &str) -> IResult<&str, &str> {
        branch::alt((
            character::streaming::digit1,
            sequence::tuple((
                combinator::opt(streaming::tag("-")),
                character::streaming::digit1,
                streaming::tag("."),
                character::streaming::digit1,
            ))
            .map(|(_, a, _, _)| a),
        ))(slice)
    }

    fn parse_identifier(slice: &str) -> IResult<&str, &str> {
        branch::alt((
            Self::parse_bool_literal,
            Self::parse_named_number_literal,
            Self::parse_constructor,
            Self::parse_constant,
        ))(slice)
    }

    fn parse_bool_literal(slice: &str) -> IResult<&str, &str> {
        branch::alt((streaming::tag("true"), streaming::tag("false")))(slice)
    }

    fn parse_named_number_literal(slice: &str) -> IResult<&str, &str> {
        sequence::tuple((
            combinator::opt(streaming::tag("-")).map(|_| ""),
            branch::alt((streaming::tag("inf"), streaming::tag("nan"))),
        ))(slice)
        .map(|_| ("", ""))
    }

    fn parse_constructor(slice: &str) -> IResult<&str, &str> {}

    fn parse_constant(slice: &str) -> IResult<&str, &str> {}

    fn parse_header_kv<'a>(
        slice: &'a str,
        resource_type: &mut String,
    ) -> IResult<&'a str, &'a str> {
        let last = Cell::new(' ');
        let resource_type_cell = Cell::new(std::mem::take(resource_type));
        let result = branch::alt((
            sequence::delimited(
                streaming::tag(" type=\""),
                streaming::take_till(|x: char| {
                    resource_type_cell.replace(resource_type_cell.take() + &x.to_string());
                    if x == '"' && last.get() != '\\' {
                        last.set(' ');
                        return true;
                    }
                    last.set(x);
                    false
                }),
                streaming::tag("\""),
            ),
            sequence::delimited(
                branch::alt((
                    streaming::tag(" name=\""),
                    streaming::tag(" parent=\""),
                    streaming::tag(" id=\""),
                )),
                streaming::take_till(|x| {
                    if x == '"' && last.get() != '\\' {
                        last.set(' ');
                        return true;
                    }
                    last.set(x);
                    false
                }),
                streaming::tag("\""),
            ),
            sequence::preceded(
                streaming::tag(" unique_id="),
                nom::character::streaming::digit1,
            ),
        ))(slice);
        *resource_type = resource_type_cell.take();
        result
    }

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
