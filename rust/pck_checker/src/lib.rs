use core::slice;
use godot::prelude::*;
use nom::branch::alt;
use nom::bytes::streaming::{tag, take_while, take_while1};
use nom::character::streaming::{char as byte, one_of};
use nom::combinator::{opt, recognize};
use nom::error::{ErrorKind, ParseError};
use nom::multi::separated_list0;
use nom::number::streaming::recognize_float;
use nom::sequence::pair;
use nom::{Err, IResult, Needed};
use std::io;
use std::{
    fs::File,
    io::{BufRead, BufReader, Read, Seek, SeekFrom},
    mem::MaybeUninit,
};
use strum::IntoEnumIterator;

mod resources;

struct MyExtension;
#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}

/// SAFETY: Implementers must have no invalid bit patterns. Or more explicitly:
/// For every possible byte sequence of length size_of::<T>(),
/// that sequence is a valid representation of T.
// yes technically a type with an empty Drop impl can be DirectlyReadable but not Copy.
unsafe trait DirectlyReadable: Copy {}

unsafe impl DirectlyReadable for u8 {}
unsafe impl DirectlyReadable for u32 {}
unsafe impl DirectlyReadable for u64 {}

const VERSION: &[u32] = &[3, 4, 6];
const BAD_EXTENSIONS: &[&str] = &[".res", ".tres", ".gd", ".gdc", ".remap", ".import"];
const UNSUPPORTED_EXTENSIONS: &[&str] = &[".scn", ".escn"];
const PARSE_CHUNK_SIZE: usize = 64;
const MAX_PARSE_CHUNK_SIZE: usize = 8 * 1024 * 1024;
const MAX_VARIANT_SIZE: usize = 512 * 1024 * 1024;
const PARSER_SENTINEL: u8 = 0;

macro_rules! unwrap_or_return {
    ($expr:expr, $default:expr, Result) => {
        match $expr {
            Ok(x) => x,
            Err(e) => {
                godot_warn!("Error at line {:?}: {}", line!(), e);
                return $default;
            }
        }
    };
    ($expr:expr, $default:expr, Option) => {
        match $expr {
            Some(x) => x,
            None => {
                godot_warn!("Error at line {:?}: unexpected None", line!());
                return $default;
            }
        }
    };
}

#[derive(GodotClass)]
#[class(init, singleton)]
struct PCKChecker {
    base: Base<Object>,
}

#[godot_api]
impl PCKChecker {
    #[func]
    fn is_pck_good(
        &self,
        pck_path: String,
        object_type_string: String,
        uuid_string: String,
    ) -> bool {
        let mut pck = BufReader::new(unwrap_or_return!(File::open(pck_path), false, Result));

        // no offset support, shouldnt be an issue
        // if some people are failing this check it might be the endianness issue in read_value
        let magic: u32 = read_value(&mut pck);
        if magic != 0x43504447 {
            godot_error!("invalid magic number, found 0x{:x}", magic);
            return false;
        }

        for v in VERSION {
            let version: u32 = read_value(&mut pck);
            if version != *v {
                godot_error!("version mismatch: got {}, expected {}", version, v);
                return false;
            }
        }

        // ignore patch version and flag8
        unwrap_or_return!(pck.seek_relative(8), false, Result);

        let file_base: u64 = read_value(&mut pck);
        let dir_offset: u64 = read_value(&mut pck);

        unwrap_or_return!(pck.seek(SeekFrom::Start(dir_offset)), false, Result);

        let file_count: u32 = read_value(&mut pck);

        let mut tscn_files: Vec<PCKFile> = Vec::with_capacity(file_count as usize);

        for _ in 0..file_count {
            let length_hint: u32 = read_value(&mut pck);
            let mut bytes = vec![0; length_hint as usize];
            unwrap_or_return!(pck.read_exact(&mut bytes), false, Result);
            let path = unwrap_or_return!(String::from_utf8(bytes), false, Result);

            let path = unwrap_or_return!(
                String::from_utf8(
                    path.into_bytes()
                        .into_iter()
                        .take_while(|x| *x != 0)
                        .collect(),
                ),
                false,
                Result
            );

            // contains("..") check appears unneeded currently, can be skipped if it causes issues
            if path.contains("..")
                || !path.starts_with(&format!(
                    "_loaded_content/{}/{}/",
                    object_type_string, uuid_string
                ))
            {
                godot_error!("found unwanted file at path {:?}", path);
                return false;
            }

            if BAD_EXTENSIONS
                .iter()
                .any(|ext| path.to_ascii_lowercase().ends_with(ext))
            {
                godot_error!("found bad extension at path {:?}", path);
                return false;
            }

            if UNSUPPORTED_EXTENSIONS
                .iter()
                .any(|ext| path.to_ascii_lowercase().ends_with(ext))
            {
                godot_error!("found unsupported extension at path {:?}", path);
                return false;
            }

            let offset: u64 = read_value(&mut pck);
            let size: u64 = read_value(&mut pck);

            // skip md5 hash and flags
            unwrap_or_return!(pck.seek_relative(20), false, Result);

            if path.ends_with(".tscn") {
                tscn_files.push(PCKFile {
                    path,
                    absolute_offset: file_base + offset,
                    size,
                });
            }
        }

        for file in &tscn_files {
            if !self.check_tscn(file, &mut pck) {
                return false;
            }
        }

        true
    }

    fn check_tscn(&self, file: &PCKFile, pck: &mut BufReader<File>) -> bool {
        unwrap_or_return!(
            pck.seek(SeekFrom::Start(file.absolute_offset)),
            false,
            Result
        );

        let mut buffer: Vec<u8> = vec![0; 19];
        unwrap_or_return!(pck.read_exact(&mut buffer), false, Result);
        let buffer = unwrap_or_return!(String::from_utf8(buffer), false, Result);
        if buffer != "[gd_scene format=4]" && buffer != "[gd_scene format=3]" {
            godot_error!("invalid header in scene {:?}. got {:?}", file.path, buffer);
            return false;
        }

        while unwrap_or_return!(pck.stream_position(), false, Result)
            < file.absolute_offset + file.size
        {
            let next: u8 = read_value(pck);

            if next <= 32 {
                continue;
            }

            if next != '[' as u8 {
                godot_error!(
                    "invalid character {:?} in scene {:?} at pos {:?}, expected '['",
                    next as char,
                    file.path,
                    pck.stream_position()
                );
                return false;
            }

            match parse_header(pck) {
                Ok(object_type) => {
                    if let Err(e) = parse_object(pck, object_type, file) {
                        godot_error!("object parse error in scene {:?}: {:?}", file.path, e);
                        let mut buffer = vec![0u8; 50];
                        let _ = pck.seek_relative(-45);
                        let _ = pck.read_exact(&mut buffer);
                        godot_error!("error occured near: {:?}", String::from_utf8_lossy(&buffer));
                        return false;
                    }
                    continue;
                }
                Err(e) => {
                    godot_error!("header parse error in scene {:?}: {:?}", file.path, e);
                    let mut buffer = vec![0u8; 50];
                    let _ = pck.seek_relative(-45);
                    let _ = pck.read_exact(&mut buffer);
                    godot_error!("error occured near: {:?}", String::from_utf8_lossy(&buffer));
                    return false;
                }
            }
        }

        return true;
    }
}

#[derive(Debug)]
enum ObjectType {
    Node,
    SubResource,
}

#[derive(Debug)]
enum HeaderParseError {
    ConnectionType,
    ExtResourceType,
    UnknownResourceType,
    GenericError,
}

#[derive(Debug)]
enum ObjectParseError {
    BadKey,
    BadValue,
    InlineOjectValue,
    BadResource,
    GenericError,
}

impl From<io::Error> for ObjectParseError {
    fn from(_: io::Error) -> Self {
        ObjectParseError::GenericError
    }
}

fn parse_header(pck: &mut BufReader<File>) -> Result<ObjectType, HeaderParseError> {
    let mut type_buffer: Vec<u8> = Vec::with_capacity(16);
    unwrap_or_return!(
        pck.read_until(' ' as u8, &mut type_buffer),
        Err(HeaderParseError::GenericError),
        Result
    );
    let type_buffer = unwrap_or_return!(
        str::from_utf8(&type_buffer),
        Err(HeaderParseError::GenericError),
        Result
    );

    match type_buffer {
        "sub_resource " => Ok(ObjectType::SubResource),
        "node " => Ok(ObjectType::Node),
        "ext_resource " => Err(HeaderParseError::ExtResourceType),
        "connection " => Err(HeaderParseError::ConnectionType),
        _ => Err(HeaderParseError::UnknownResourceType),
    }
}

fn parse_object(
    pck: &mut BufReader<File>,
    object_type: ObjectType,
    file: &PCKFile,
) -> Result<(), ObjectParseError> {
    match object_type {
        ObjectType::Node => parse_node(pck, file),
        ObjectType::SubResource => {
            let mut buffer: Vec<u8> = Vec::with_capacity(6);
            unwrap_or_return!(
                pck.read_until('"' as u8, &mut buffer),
                Err(ObjectParseError::GenericError),
                Result
            );
            let buffer = unwrap_or_return!(
                String::from_utf8(buffer),
                Err(ObjectParseError::GenericError),
                Result
            );

            if buffer != "type=\"" {
                return Err(ObjectParseError::GenericError);
            }

            let mut buffer: Vec<u8> = Vec::new();
            unwrap_or_return!(
                pck.read_until('"' as u8, &mut buffer),
                Err(ObjectParseError::GenericError),
                Result
            );
            let buffer = unwrap_or_return!(
                String::from_utf8(buffer),
                Err(ObjectParseError::GenericError),
                Result
            );
            let buffer = &buffer[..buffer.len() - 1];
            if !(resources::ResourceWhitelist::iter().any(|f| f.to_string() == buffer)) {
                return Err(ObjectParseError::BadResource);
            }
            unwrap_or_return!(
                pck.skip_until(']' as u8),
                Err(ObjectParseError::GenericError),
                Result
            );
            loop {
                let mut next: u8 = read_value(pck);
                loop {
                    if next > 32 {
                        break;
                    }
                    next = read_value(pck);
                }

                unwrap_or_return!(
                    pck.seek_relative(-1),
                    Err(ObjectParseError::GenericError),
                    Result
                );

                if next == '[' as u8 || next == 0 {
                    return Ok(());
                }

                if unwrap_or_return!(
                    pck.stream_position(),
                    Err(ObjectParseError::GenericError),
                    Result
                ) > file.size + file.absolute_offset
                {
                    return Ok(());
                }

                if let Err(e) = parse_value(pck) {
                    return Err(e);
                }
            }
        }
    }
}

fn parse_node(pck: &mut BufReader<File>, file: &PCKFile) -> Result<(), ObjectParseError> {
    unwrap_or_return!(
        pck.skip_until(']' as u8),
        Err(ObjectParseError::GenericError),
        Result
    );
    loop {
        let mut next: u8 = read_value(pck);
        loop {
            if next > 32 {
                break;
            }
            next = read_value(pck);
        }

        unwrap_or_return!(
            pck.seek_relative(-1),
            Err(ObjectParseError::GenericError),
            Result
        );

        if next == '[' as u8 {
            return Ok(());
        }

        if unwrap_or_return!(
            pck.stream_position(),
            Err(ObjectParseError::GenericError),
            Result
        ) > file.size + file.absolute_offset
        {
            return Ok(());
        }

        if let Err(e) = parse_value(pck) {
            return Err(e);
        }
    }
}

fn parse_value(pck: &mut BufReader<File>) -> Result<(), ObjectParseError> {
    let mut buffer: Vec<u8> = Vec::new();
    unwrap_or_return!(
        pck.read_until('=' as u8, &mut buffer),
        Err(ObjectParseError::GenericError),
        Result
    );

    let tmp_string = String::from_utf8_lossy(&buffer[..buffer.len() - 2]).to_string();

    if buffer[..buffer.len() - 2]
        .array_windows::<2>()
        .any(|c| c[1] == '"' as u8 && c[0] != '\\' as u8)
        && !(tmp_string.starts_with('"')
            && tmp_string.ends_with('"')
            && !tmp_string.ends_with("\\\""))
    {
        return Err(ObjectParseError::BadKey);
    }

    let next: u8 = read_value(pck);
    if next != ' ' as u8 {
        return Err(ObjectParseError::BadKey);
    }

    parse_variant(pck)
}

// ai usage: this function along with its helpers are all primarily ai generated
fn parse_variant(reader: &mut BufReader<File>) -> Result<(), ObjectParseError> {
    let mut buf: Vec<u8> = Vec::with_capacity(PARSE_CHUNK_SIZE);
    let mut bytes_read = 0usize;
    let mut chunk_size = PARSE_CHUNK_SIZE;
    let mut at_eof = false;

    loop {
        match value_at_cursor(&buf) {
            Ok((rest, ())) => {
                // The sentinel is never consumed by a successful parse, so this
                // only counts bytes that really came from the reader.
                let consumed = buf.len() - rest.len();
                let unconsumed = bytes_read.saturating_sub(consumed);
                reader.seek_relative(-(unconsumed as i64))?;
                return Ok(());
            }
            Err(Err::Error(VariantError::InlineObject))
            | Err(Err::Failure(VariantError::InlineObject)) => {
                return Err(ObjectParseError::InlineOjectValue);
            }
            Err(Err::Error(_)) | Err(Err::Failure(_)) => {
                return Err(ObjectParseError::BadValue);
            }
            Err(Err::Incomplete(_)) => {
                if at_eof {
                    // Nothing left to read and the value is still unterminated.
                    return Err(ObjectParseError::BadValue);
                }
                if bytes_read >= MAX_VARIANT_SIZE {
                    return Err(ObjectParseError::BadValue);
                }
                buf.truncate(bytes_read);
                buf.resize(bytes_read + chunk_size, 0);
                let n = reader.read(&mut buf[bytes_read..])?;
                bytes_read += n;
                buf.truncate(bytes_read);
                if n == 0 {
                    at_eof = true;
                    buf.push(PARSER_SENTINEL);
                } else {
                    // A refill restarts the parse at the beginning of the value,
                    // so a fixed chunk size makes a multi megabyte value
                    // quadratic: ~4000 full re-parses for 4 MB. Doubling keeps
                    // the number of passes logarithmic (~13 for 4 MB) and the
                    // total work linear in the size of the value.
                    chunk_size = chunk_size.saturating_mul(2).min(MAX_PARSE_CHUNK_SIZE);
                }
            }
        }
    }
}

/// Internal parse error. Deliberately does not borrow the input so that it can
/// be carried out of the borrowed scratch buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VariantError {
    Syntax,
    InlineObject,
}

impl<I> ParseError<I> for VariantError {
    fn from_error_kind(_: I, _: ErrorKind) -> Self {
        VariantError::Syntax
    }

    fn append(_: I, _: ErrorKind, other: Self) -> Self {
        other
    }
}

type PResult<'a, T = ()> = IResult<&'a [u8], T, VariantError>;

fn value_at_cursor(i: &[u8]) -> PResult<'_> {
    let (i, _) = ws(i)?;
    let (rest, ()) = variant(i)?;
    // A value has to be followed by a delimiter, otherwise `42abc` would
    // happily parse as `42` and leave the caller on garbage.
    match rest.first() {
        None => Err(Err::Incomplete(Needed::new(1))),
        Some(&b) if is_terminator(b) => Ok((rest, ())),
        Some(_) => Err(Err::Error(VariantError::Syntax)),
    }
}

fn is_terminator(b: u8) -> bool {
    b == PARSER_SENTINEL || is_ws(b) || matches!(b, b'#' | b',' | b':' | b')' | b']' | b'}')
}

fn is_ws(b: u8) -> bool {
    matches!(b, b' ' | b'\t' | b'\r' | b'\n')
}

/// Skips insignificant whitespace. Never called in a position where trailing
/// whitespace would be swallowed into the returned value.
fn ws(i: &[u8]) -> PResult<'_> {
    let (i, _) = take_while(is_ws)(i)?;
    Ok((i, ()))
}

fn variant(i: &[u8]) -> PResult<'_> {
    alt((quoted_variant, array, dictionary, number, named))(i)
}

/// `"String"`, `&"StringName"`, `^"NodePath"`.
fn quoted_variant(i: &[u8]) -> PResult<'_> {
    let (i, _) = opt(one_of("&^"))(i)?;
    quoted_string(i)
}

fn quoted_string(i: &[u8]) -> PResult<'_> {
    let (mut rest, _) = byte('"')(i)?;
    loop {
        match rest.first() {
            None => return Err(Err::Incomplete(Needed::new(1))),
            Some(b'"') => return Ok((&rest[1..], ())),
            Some(b'\\') => {
                if rest.len() < 2 {
                    return Err(Err::Incomplete(Needed::new(1)));
                }
                rest = &rest[2..];
            }
            // Unterminated string at EOF.
            Some(&PARSER_SENTINEL) => return Err(Err::Error(VariantError::Syntax)),
            Some(_) => rest = &rest[1..],
        }
    }
}

fn number(i: &[u8]) -> PResult<'_> {
    let (i, _) = opt(one_of("+-"))(i)?;
    let (i, _) = alt((tag("inf"), tag("nan"), recognize_float))(i)?;
    Ok((i, ()))
}

fn array(i: &[u8]) -> PResult<'_> {
    let (i, _) = byte('[')(i)?;
    let (i, _) = ws(i)?;
    let (i, _) = separated_list0(comma, variant)(i)?;
    let (i, _) = opt(pair(ws, byte(',')))(i)?;
    let (i, _) = ws(i)?;
    let (i, _) = byte(']')(i)?;
    Ok((i, ()))
}

fn dictionary(i: &[u8]) -> PResult<'_> {
    let (i, _) = byte('{')(i)?;
    let (i, _) = ws(i)?;
    let (i, _) = separated_list0(comma, entry)(i)?;
    let (i, _) = opt(pair(ws, byte(',')))(i)?;
    let (i, _) = ws(i)?;
    let (i, _) = byte('}')(i)?;
    Ok((i, ()))
}

fn entry(i: &[u8]) -> PResult<'_> {
    let (i, ()) = variant(i)?;
    let (i, _) = ws(i)?;
    let (i, _) = byte(':')(i)?;
    let (i, _) = ws(i)?;
    variant(i)
}

fn comma(i: &[u8]) -> PResult<'_> {
    let (i, _) = ws(i)?;
    let (i, _) = byte(',')(i)?;
    let (i, _) = ws(i)?;
    Ok((i, ()))
}

/// Anything that starts with an identifier: keywords (`null`, `true`, `inf`),
/// constructors (`Vector2(..)`, `ExtResource(..)`, `PackedByteArray(..)`),
/// constants (`Color.RED`) and typed containers (`Array[int]([..])`).
fn named(i: &[u8]) -> PResult<'_> {
    let (after_name, name) = ident(i)?;

    // Keywords are complete values on their own and Godot's parser resolves
    // them without looking at the next token, so this must happen before the
    // delimiter probe below. Probing first would let the `[` of the following
    // `[sub_resource ...]` header be read as the type arguments of a value like
    // `ssao_enabled = true`, swallowing the rest of the scene.
    if matches!(
        name,
        b"null" | b"true" | b"false" | b"inf" | b"nan" | b"inf_neg"
    ) {
        // Note: `after_name`, not a whitespace skipped probe - trailing
        // whitespace stays for the caller to terminate on.
        return Ok((after_name, ()));
    }

    // Look past whitespace without committing to consuming it.
    let (probe, _) = ws(after_name)?;

    match probe.first() {
        None => Err(Err::Incomplete(Needed::new(1))),
        Some(b'(') => {
            if name == b"Object" {
                // `Failure` is unrecoverable in nom, so no enclosing `alt`,
                // `opt` or `separated_list0` can backtrack over it. That is what
                // makes nested inline objects impossible to miss.
                return Err(Err::Failure(VariantError::InlineObject));
            }
            if PACKED_ARRAYS.contains(&name) {
                return packed_array(probe);
            }
            call(probe)
        }
        Some(b'[') => {
            let (rest, _) = type_arguments(probe)?;
            let (rest, _) = ws(rest)?;
            call(rest)
        }
        Some(b'.') => constant_path(probe),
        // A bare identifier that is not a keyword is not a value.
        _ => Err(Err::Error(VariantError::Syntax)),
    }
}

/// Types whose payload is a flat run of numbers - plus, for `PackedByteArray`
/// in scene format 4, a single base64 string. Godot keeps mesh, collision and
/// navigation data in these, so they are the only values that regularly reach
/// several megabytes, and the only ones worth taking out of the generic
/// element machinery below.
const PACKED_ARRAYS: &[&[u8]] = &[
    b"PackedByteArray",
    b"PackedInt32Array",
    b"PackedInt64Array",
    b"PackedFloat32Array",
    b"PackedFloat64Array",
    b"PackedVector2Array",
    b"PackedVector3Array",
    b"PackedVector4Array",
    b"PackedColorArray",
];

/// The `(...)` of a `PACKED_ARRAYS` value, scanned in a single pass.
///
/// `call` would run every element through `separated_list0`, `alt` and
/// `recognize_float`, which for the ~800k elements of a 4 MB `PackedByteArray`
/// costs more than the rest of the scene put together. None of these payloads
/// can contain a nested value, so validating the bytes and looking for the
/// closing paren is equivalent here and roughly an order of magnitude cheaper -
/// runs of payload bytes are skipped a whole token at a time.
///
/// The payload is only checked lexically: `PackedByteArray(1..2)` is accepted
/// here and rejected by Godot's own parser at load time. The property that
/// matters is that no `(` can appear in it, so it can neither hide an inline
/// object from `named` nor shift the closing paren.
fn packed_array(i: &[u8]) -> PResult<'_> {
    let (mut rest, _) = byte('(')(i)?;
    loop {
        match rest.first() {
            None => return Err(Err::Incomplete(Needed::new(1))),
            Some(b')') => return Ok((&rest[1..], ())),
            // Scene format 4 writes the whole payload as one base64 string.
            Some(b'"') => rest = quoted_string(rest)?.0,
            // The only non-numeric tokens Godot writes into a float payload.
            Some(b'i') | Some(b'n') => {
                rest = alt((tag("inf_neg"), tag("inf"), tag("nan")))(rest)?.0
            }
            Some(&b) if is_packed_payload(b) => {
                match rest.iter().position(|&b| !is_packed_payload(b)) {
                    // The run is still going when the buffer ends, so the token
                    // it ends on may continue into bytes we have not read yet.
                    None => return Err(Err::Incomplete(Needed::new(1))),
                    Some(n) => rest = &rest[n..],
                }
            }
            _ => return Err(Err::Error(VariantError::Syntax)),
        }
    }
}

/// Bytes that may appear between the parens of a `PACKED_ARRAYS` value: numbers
/// and the commas and whitespace separating them. Excludes the sentinel, so a
/// payload truncated at EOF is a syntax error rather than a valid parse.
fn is_packed_payload(b: u8) -> bool {
    b.is_ascii_digit() || is_ws(b) || matches!(b, b',' | b'+' | b'-' | b'.' | b'e' | b'E')
}

fn call(i: &[u8]) -> PResult<'_> {
    let (i, _) = byte('(')(i)?;
    let (i, _) = ws(i)?;
    let (i, _) = separated_list0(comma, variant)(i)?;
    let (i, _) = opt(pair(ws, byte(',')))(i)?;
    let (i, _) = ws(i)?;
    let (i, _) = byte(')')(i)?;
    Ok((i, ()))
}

/// `Color.RED`, `Vector3.UP`, ...
fn constant_path(i: &[u8]) -> PResult<'_> {
    let mut probe = i;
    loop {
        let (rest, _) = byte('.')(probe)?;
        let (rest, _) = ws(rest)?;
        let (after_ident, _) = ident(rest)?;
        let (next, _) = ws(after_ident)?;
        match next.first() {
            None => return Err(Err::Incomplete(Needed::new(1))),
            Some(b'.') => probe = next,
            _ => return Ok((after_ident, ())),
        }
    }
}

/// The `[...]` of a typed container, consumed verbatim. The contents are type
/// names rather than values, so `Array[Object]([..])` is legal and is *not*
/// reported as an inline object.
fn type_arguments(i: &[u8]) -> PResult<'_> {
    let (mut rest, _) = byte('[')(i)?;
    let mut depth = 1usize;
    loop {
        match rest.first() {
            None => return Err(Err::Incomplete(Needed::new(1))),
            Some(b'"') => {
                let (next, ()) = quoted_string(rest)?;
                rest = next;
            }
            Some(b'[') => {
                depth += 1;
                rest = &rest[1..];
            }
            Some(b']') => {
                depth -= 1;
                rest = &rest[1..];
                if depth == 0 {
                    return Ok((rest, ()));
                }
            }
            Some(&PARSER_SENTINEL) => return Err(Err::Error(VariantError::Syntax)),
            Some(_) => rest = &rest[1..],
        }
    }
}

fn ident(i: &[u8]) -> PResult<'_, &[u8]> {
    recognize(pair(
        take_while1(|b: u8| b.is_ascii_alphabetic() || b == b'_'),
        take_while(|b: u8| b.is_ascii_alphanumeric() || b == b'_'),
    ))(i)
}

// end of ai generated code

#[derive(Debug)]
struct PCKFile {
    path: String,
    absolute_offset: u64,
    size: u64,
}

// this is not very portable but it works on my machine and it can always be replaced later
// machines with a different endianness will error out at the magic number check
fn read_value<T: DirectlyReadable>(file: &mut impl Read) -> T {
    unsafe {
        let mut value = MaybeUninit::zeroed().assume_init();

        let bytes = slice::from_raw_parts_mut(&raw mut value as *mut u8, std::mem::size_of::<T>());

        // partial write is not an issue since we will never be invalid
        unwrap_or_return!(file.read_exact(bytes), value, Result);

        value
    }
}
