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

const VERSION: [u32; 3] = [3, 4, 6];
const BAD_EXTENSIONS: &[&str] = &[".res", ".tres", ".gd"];
const UNSUPPORTED_EXTENSIONS: &[&str] = &[".scn", ".escn"];

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
        let mut pck = BufReader::new(File::open(pck_path).unwrap());

        // no offset support, shouldnt be an issue
        let magic: u32 = unsafe { read_value(&mut pck) };
        if magic != 0x43504447 {
            godot_error!("invalid magic number, found 0x{:x}", magic);
            return false;
        }

        for v in VERSION {
            let version: u32 = unsafe { read_value(&mut pck) };
            if version != v {
                godot_error!("version mismatch: got {}, expected {}", version, v);
                return false;
            }
        }
        // ignore patch version and flag8
        pck.seek_relative(8).unwrap();

        let file_base: u64 = unsafe { read_value(&mut pck) };
        let dir_offset: u64 = unsafe { read_value(&mut pck) };

        pck.seek(SeekFrom::Start(dir_offset)).unwrap();

        let file_count: u32 = unsafe { read_value(&mut pck) };

        let mut tscn_files: Vec<PCKFile> = Vec::with_capacity(file_count as usize);

        for _ in 0..file_count {
            let length_hint: u32 = unsafe { read_value(&mut pck) };
            let mut bytes = vec![0; length_hint as usize];
            pck.read_exact(&mut bytes).unwrap();
            let path = String::from_utf8(bytes).unwrap();

            let path =
                String::from_utf8(path.into_bytes().into_iter().filter(|x| *x != 0).collect())
                    .unwrap();

            if !path.starts_with(&format!(
                "_loaded_content/{}/{}/",
                object_type_string, uuid_string
            )) {
                godot_error!("found unwanted file at path {:?}", path);
                return false;
            }

            if BAD_EXTENSIONS.iter().any(|ext| path.ends_with(ext)) {
                godot_error!("found bad extension at path {:?}", path);
                return false;
            }

            if UNSUPPORTED_EXTENSIONS.iter().any(|ext| path.ends_with(ext)) {
                godot_error!("found unsupported extension at path {:?}", path);
                return false;
            }

            let offset: u64 = unsafe { read_value(&mut pck) };
            let size: u64 = unsafe { read_value(&mut pck) };
            // skip md5 hash and flags
            pck.seek_relative(20).unwrap();
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
        enum ParserState {
            Scanning,
            InHeader,
            InResource,
        }

        pck.seek(SeekFrom::Start(file.absolute_offset)).unwrap();

        let buffer: [u8; 19] = unsafe { read_value(pck) };
        let buffer: String = buffer
            .into_iter()
            .map(|c| char::from_u32(c as u32).unwrap())
            .collect();
        if buffer != "[gd_scene format=4]" && buffer != "[gd_scene format=3]" {
            godot_error!("invalid header in scene {:?}. got {:?}", file.path, buffer);
            return false;
        }

        let mut state = ParserState::Scanning;

        while pck.stream_position().unwrap() < file.absolute_offset + file.size {
            match state {
                ParserState::Scanning => {
                    let buffer: u8 = unsafe { read_value(pck) };
                    let buffer = char::from_u32(buffer as u32).unwrap();
                    if buffer == '[' {
                        state = ParserState::InHeader;
                    } else {
                        if buffer != '\n' {
                            pck.skip_until('\n' as u8).unwrap();
                        }
                    }
                }
                ParserState::InHeader => {
                    let mut type_buffer: Vec<u8> = Vec::new();
                    pck.read_until(' ' as u8, &mut type_buffer).unwrap();
                    let type_buffer = str::from_utf8(&type_buffer).unwrap();
                    state = match type_buffer {
                        "sub_resource " => ParserState::InResource,
                        "node " => ParserState::Scanning,
                        _ => {
                            godot_error!(
                                "external resource, connection, or unknown type '{:?}' in {:?}",
                                type_buffer,
                                file.path
                            );
                            return false;
                        }
                    };
                }
                ParserState::InResource => {
                    let mut buffer: Vec<u8> = Vec::new();
                    pck.read_until('"' as u8, &mut buffer).unwrap();
                    let buffer = str::from_utf8(&buffer).unwrap();
                    if buffer != "type=\"" {
                        godot_error!(
                            "expected type declaration got '{:?}' in {:?}",
                            buffer,
                            file.path
                        );
                        return false;
                    }

                    let mut buffer: Vec<u8> = Vec::new();
                    pck.read_until('"' as u8, &mut buffer).unwrap();
                    let buffer = str::from_utf8(&buffer[..buffer.len() - 1]).unwrap();
                    if !(resources::ResourceWhitelist::iter().any(|f| f.to_string() == buffer)) {
                        godot_error!(
                            "unrecognized resource type '{:?}' in {:?}",
                            buffer,
                            file.path
                        );
                        return false;
                    }
                    state = ParserState::Scanning;
                }
            }
        }

        return true;
    }
}

#[derive(Debug)]
struct PCKFile {
    path: String,
    absolute_offset: u64,
    size: u64,
}

/// Safety: T must have no invalid bit patterns
unsafe fn read_value<T: Copy>(file: &mut impl Read) -> T {
    unsafe {
        let mut value = MaybeUninit::zeroed().assume_init();

        let bytes =
            slice::from_raw_parts_mut(&mut value as *mut T as *mut u8, std::mem::size_of::<T>());

        file.read_exact(bytes).unwrap();

        value
    }
}
