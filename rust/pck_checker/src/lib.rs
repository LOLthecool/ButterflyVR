use core::slice;
use godot::prelude::*;
use std::{
    fs::File,
    io::{BufReader, Read, Seek, SeekFrom},
    mem::MaybeUninit,
};

mod objects;
mod parser;
mod tests;

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
// todo: consider replacing this with a whitelist
const BAD_EXTENSIONS: &[&str] = &[
    ".res",
    ".tres",
    ".gd",
    ".gdc",
    ".remap",
    ".import",
    ".gdextension",
    ".escn",
];
const UNSUPPORTED_EXTENSIONS: &[&str] = &[".scn"];

#[macro_export]
macro_rules! unwrap_or_return {
    ($expr:expr, $default:expr, Result) => {
        match $expr {
            Ok(x) => x,
            Err(e) => {
                godot_warn!(
                    "Error at line {:?} in {:?}: {}",
                    line!(),
                    core::panic::Location::caller(),
                    e
                );
                return $default;
            }
        }
    };
    ($expr:expr, $default:expr, Option) => {
        match $expr {
            Some(x) => x,
            None => {
                godot_warn!(
                    "Error at line {:?} in {:?}: unexpected None",
                    line!(),
                    core::panic::Location::caller()
                );
                return $default;
            }
        }
    };
}

#[derive(GodotClass)]
#[class(init, singleton)]
pub struct PCKChecker {
    base: Base<Object>,
}

#[godot_api]
impl PCKChecker {
    #[func]
    pub fn is_pck_good(pck_path: String, object_type_string: String, uuid_string: String) -> bool {
        let mut pck = BufReader::new(unwrap_or_return!(File::open(pck_path), false, Result));

        // no offset support, shouldnt be an issue
        // if some people are failing this check it might be the endianness issue in read_value
        let magic: u32 = read_value(&mut pck);
        if magic != 0x43504447 {
            godot_error!("invalid magic number, found 0x{:x}", magic);
            return false;
        }

        // VERSION is singular, we check for a specific version using multiple reads not multiple versions
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

            // todo: binary scene parser
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

            // this should be the only 'important' file type
            if path.to_lowercase().ends_with(".tscn") {
                if !parser::check_tscn(file_base + offset, size, &mut pck) {
                    godot_warn!("failed to parse tscn at path {:?}", path);
                    return false;
                }
            }
        }

        true
    }
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
