use godot::global::{godot_error, godot_warn};
use nom::bytes::streaming;
use nom::sequence;
use nom::{self, IResult, character};
use nom::{branch, combinator};
use std::cell::Cell;
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
};
use strum::IntoEnumIterator;

use crate::{objects, unwrap_or_return};

const PARSE_CHUNK_SIZE: usize = 1024 * 1024 * 50;
const MAX_PARSE_CHUNK_SIZE: usize = 1024 * 1024 * 1000;

macro_rules! handle_incomplete {
    ($result:expr) => {
        match $result {
            Ok(x) => x,
            Err(nom::Err::Incomplete(_)) => return None,
            Err(e) => {
                godot_warn!(
                    "parse error at line {:?}: {:?}",
                    line!(),
                    e.map_input(|_| Option::<&str>::None)
                );
                return Some(false);
            }
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
            if !parser.parse_buffer() {
                let _ = parser.parsing_completed();
                godot_warn!("scene failed to parse!");
                return false;
            }

            parser.clean_trailing_whitespace();
            if !parser.parsing_completed() {
                godot_error!("reached end of file but failed to parse final value");
                return false;
            }
            return true;
        }

        parser.add_to_buffer(&buffer[..bytes_read]);

        if !parser.parse_buffer() {
            let _ = parser.parsing_completed();
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

    fn clean_trailing_whitespace(&mut self) {
        self.buffer.retain(|&c| c >= 33);
    }

    fn parse_buffer(&mut self) -> bool {
        if self.extend_failed {
            godot_warn!("failed to extend buffer");
            return false;
        }
        if self.buffer.len() == 0 {
            return true;
        }

        let mut slice = &*self.buffer;
        let mut last_slice_len: usize = 0;
        let mut last_state: ParserState = ParserState::SceneHeader;

        loop {
            if slice.len() == last_slice_len && self.state == last_state {
                godot_warn!(
                    "infinite loop while parsing (did you forget to update the slice after consuming?)"
                );
                return false;
            }
            last_slice_len = slice.len();
            last_state = self.state.clone();
            let result = match self.state {
                ParserState::SceneHeader => Self::parse_scene_header(&mut slice, &mut self.state),
                ParserState::ObjectHeader => Self::parse_object_header(&mut slice, &mut self.state),
                ParserState::ObjectValue => {
                    Self::parse_object_key_value(&mut slice, &mut self.state)
                }
            };
            match result {
                Some(true) => continue, // parsed without issue
                Some(false) => {
                    const WIDE_TAIL_LENGTH: usize = 50;
                    const WIDE_HEAD_LENGTH: usize = 50;
                    const TAIL_LENGTH: usize = 10;
                    const HEAD_LENGTH: usize = 10;
                    godot_warn!(
                        "parse error around\n{:?}\nat or near: {:?}\non char {:?}",
                        String::from_utf8_lossy(
                            &self.buffer[(self.buffer.len() - slice.len())
                                .saturating_sub(WIDE_HEAD_LENGTH)
                                ..((self.buffer.len() - slice.len()) + WIDE_TAIL_LENGTH)
                                    .min(self.buffer.len())]
                        ),
                        String::from_utf8_lossy(
                            &self.buffer[(self.buffer.len() - slice.len())
                                .saturating_sub(HEAD_LENGTH)
                                ..((self.buffer.len() - slice.len()) + TAIL_LENGTH)
                                    .min(self.buffer.len())]
                        ),
                        String::from_utf8_lossy(&slice[..1])
                    );
                    return false;
                }
                None => {
                    // reached end of buffer before finishing
                    self.buffer.drain(..self.buffer.len() - slice.len());
                    return true;
                }
            }
        }
    }
    fn parse_scene_header(slice: &mut &[u8], state: &mut ParserState) -> Option<bool> {
        let Ok(str_slice) = str::from_utf8(&slice) else {
            // either file is malformed or a character got cut off at the end of the slice
            // if malformed will error out once buffer is filled or file is fully read
            return None;
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
            // either file is malformed or a character got cut off at the end of the slice
            // if malformed will error out once buffer is filled or file is fully read
            return None;
        };

        let (str_slice, _) = handle_incomplete!(character::streaming::multispace0::<
            &str,
            nom::error::Error<&str>,
        >(str_slice));

        let (str_slice, object_type) = handle_incomplete!(branch::alt((
            streaming::tag::<&str, &str, nom::error::Error<_>>("[sub_resource"),
            streaming::tag("[node"),
        ))(str_slice));

        let mut str_slice = str_slice;
        let mut resource_type = String::new();

        loop {
            let result = handle_incomplete!(branch::alt((
                |str_slice| Self::parse_header_kv(str_slice, &mut resource_type),
                streaming::tag("]"),
            ))(str_slice));

            str_slice = result.0;
            let taken = result.1;

            if taken == "]" {
                if object_type == "[sub_resource" {
                    if resource_type.is_empty() {
                        godot_warn!("found sub_resource with no type header");
                        return Some(false);
                    } else {
                        if objects::ObjectWhitelist::iter()
                            .all(|x| resource_type != x.to_string().as_str())
                        {
                            godot_warn!("found disallowed sub_resource type: {}", resource_type);
                            return Some(false);
                        }
                    }
                }
                *state = ParserState::ObjectValue;
                *slice = &slice[slice.len() - str_slice.as_bytes().len()..];
                return Some(true);
            }
        }
    }
    fn parse_object_key_value(slice: &mut &[u8], state: &mut ParserState) -> Option<bool> {
        let Ok(str_slice) = str::from_utf8(&slice) else {
            // either file is malformed or a character got cut off at the end of the slice
            // if malformed will error out once buffer is filled or file is fully read
            return None;
        };

        let (str_slice, _) = handle_incomplete!(character::streaming::multispace0::<
            &str,
            nom::error::Error<&str>,
        >(str_slice));

        match streaming::tag::<&str, &str, nom::error::Error<&str>>("[")(str_slice) {
            // dont update str_slice since header parsing expects leading [
            Ok((_, _)) => {
                *state = ParserState::ObjectHeader;
                *slice = &slice[slice.len() - str_slice.as_bytes().len()..];
                return Some(true);
            }
            Err(nom::Err::Incomplete(_)) => {
                return None;
            }
            Err(_) => {}
        };

        let (str_slice, _) = handle_incomplete!(combinator::recognize(sequence::terminated(
            branch::alt((
                combinator::recognize(nom::multi::many1(branch::alt((
                    character::streaming::alphanumeric1::<&str, nom::error::Error<&str>>,
                    streaming::tag("_"),
                    streaming::tag("/"),
                    streaming::tag("."),
                )))),
                combinator::recognize(Self::parse_string)
            )),
            streaming::tag(" = "),
        ),)(str_slice));

        let (str_slice, _) = handle_incomplete!(Self::parse_value(str_slice));

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

    fn parse_container(slice: &str) -> IResult<&str, &str> {
        branch::alt((Self::parse_array, Self::parse_dictionary))(slice)
    }

    fn parse_array(slice: &str) -> IResult<&str, &str> {
        combinator::recognize(sequence::delimited(
            sequence::pair(streaming::tag("["), character::streaming::multispace0),
            nom::multi::separated_list0(
                sequence::pair(streaming::tag(","), character::streaming::multispace0),
                Self::parse_value,
            ),
            sequence::pair(character::streaming::multispace0, streaming::tag("]")),
        ))(slice)
    }

    fn parse_dictionary(slice: &str) -> IResult<&str, &str> {
        combinator::recognize(sequence::delimited(
            sequence::pair(streaming::tag("{"), character::streaming::multispace0),
            nom::multi::separated_list0(
                sequence::pair(streaming::tag(","), character::streaming::multispace0),
                sequence::separated_pair(
                    Self::parse_value,
                    sequence::pair(streaming::tag(":"), character::streaming::multispace0),
                    Self::parse_value,
                ),
            ),
            sequence::pair(character::streaming::multispace0, streaming::tag("}")),
        ))(slice)
    }

    fn parse_number_literal(slice: &str) -> IResult<&str, &str> {
        combinator::recognize(sequence::tuple((
            combinator::recognize(combinator::opt(streaming::tag("-"))),
            branch::alt((
                streaming::tag("inf"),
                streaming::tag("nan"),
                nom::number::streaming::recognize_float,
            )),
        )))(slice)
    }

    fn parse_identifier(slice: &str) -> IResult<&str, &str> {
        branch::alt((
            streaming::tag("null"),
            Self::parse_bool_literal,
            Self::parse_typed_container,
            Self::parse_constructor,
        ))(slice)
    }

    fn parse_bool_literal(slice: &str) -> IResult<&str, &str> {
        branch::alt((streaming::tag("true"), streaming::tag("false")))(slice)
    }

    fn parse_typed_container(slice: &str) -> IResult<&str, &str> {
        branch::alt((
            combinator::recognize(sequence::tuple((
                streaming::tag("Array"),
                sequence::delimited(
                    streaming::tag("["),
                    nom::multi::many1_count(branch::alt((
                        character::streaming::alphanumeric1::<&str, nom::error::Error<&str>>,
                        combinator::recognize(character::streaming::char('_')),
                    ))),
                    streaming::tag("]"),
                ),
                sequence::delimited(streaming::tag("("), Self::parse_array, streaming::tag(")")),
            ))),
            combinator::recognize(sequence::tuple((
                streaming::tag("Dictionary"),
                sequence::delimited(
                    streaming::tag("["),
                    sequence::separated_pair(
                        nom::multi::many1_count(branch::alt((
                            character::streaming::alphanumeric1::<&str, nom::error::Error<&str>>,
                            combinator::recognize(character::streaming::char('_')),
                        ))),
                        streaming::tag(", "),
                        nom::multi::many1_count(branch::alt((
                            character::streaming::alphanumeric1::<&str, nom::error::Error<&str>>,
                            combinator::recognize(character::streaming::char('_')),
                        ))),
                    ),
                    streaming::tag("]"),
                ),
                sequence::delimited(
                    streaming::tag("("),
                    Self::parse_dictionary,
                    streaming::tag(")"),
                ),
            ))),
        ))(slice)
    }

    fn parse_constructor(slice: &str) -> IResult<&str, &str> {
        let (str_slice, name) = character::streaming::alphanumeric1(slice)?;
        match name {
            "Object" | "Resource" | "ExtResource" => {
                godot_error!("found inline object during parse!");
                Err(nom::Err::Failure(nom::error::Error::new(
                    slice,
                    nom::error::ErrorKind::Tag,
                )))
            }
            "PackedByteArray" => combinator::recognize(sequence::delimited(
                streaming::tag("(\""),
                nom::multi::many0_count(branch::alt((
                    character::streaming::alphanumeric1,
                    streaming::tag("+"),
                    streaming::tag("/"),
                ))),
                sequence::pair(
                    combinator::recognize(nom::multi::many0_count(streaming::tag("="))),
                    streaming::tag("\")"),
                ),
            ))(str_slice),
            _ => combinator::recognize(sequence::delimited(
                streaming::tag("("),
                nom::multi::separated_list0(
                    sequence::pair(streaming::tag(","), character::streaming::multispace0),
                    Self::parse_value,
                ),
                streaming::tag(")"),
            ))(str_slice),
        }
    }

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
                    if x == '"' && last.get() != '\\' {
                        last.set(' ');
                        return true;
                    }
                    resource_type_cell.replace(resource_type_cell.take() + &x.to_string());
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
        if !result {
            godot_warn!(
                "parsing not complete, remaining: {:?}",
                &self.buffer[..100.min(self.buffer.len())]
                    .iter()
                    .map(|x| *x as char)
                    .collect::<String>()
            );
            self.buffer.clear();
        }
        return result;
    }
}

impl Drop for TSCNParser {
    fn drop(&mut self) {
        if !self.parsing_completed() {
            godot_warn!("tried to drop parser but we never checked if parsing finished!")
        }
    }
}
