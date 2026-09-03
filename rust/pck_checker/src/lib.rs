use core::slice;
use godot::prelude::*;
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

macro_rules! unwrap_or_return {
    ($expr:expr, $default:expr, Result) => {
        match $expr {
            Ok(x) => x,
            Err(e) => {
                godot_warn!("Error at line {}: {}", e, line!());
                return $default;
            }
        }
    };
    ($expr:expr, $default:expr, Option) => {
        match $expr {
            Some(x) => x,
            None => {
                godot_warn!("Error at line {}: unexpected None", line!());
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
                    "invalid character {:?} in scene {:?}, expected '['",
                    next as char,
                    file.path
                );
                return false;
            }

            match parse_header(pck) {
                Ok(object_type) => {
                    if let Err(e) = parse_object(pck, object_type) {
                        godot_error!("object parse error in scene {:?}: {:?}", file.path, e);
                        return false;
                    }
                    continue;
                }
                Err(e) => {
                    godot_error!("header parse error in scene {:?}: {:?}", file.path, e);
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
) -> Result<(), ObjectParseError> {
    match object_type {
        ObjectType::Node => parse_node(pck),
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
            Ok(())
        }
    }
}

fn parse_node(pck: &mut BufReader<File>) -> Result<(), ObjectParseError> {
    Ok(())
}

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
