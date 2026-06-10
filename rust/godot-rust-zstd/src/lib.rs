use std::fs::File;
use std::io::{Read, Write};

use godot::prelude::*;
use zstd::{Decoder, Encoder};

const CHUNK_SIZE: usize = 1024 * 1024;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}

#[derive(GodotClass)]
#[class(init, singleton)]
struct ZSTDCompressor {
    base: Base<Object>,
}
#[godot_api]
impl ZSTDCompressor {
    #[func]
    fn compress_file_to_file(&mut self, input_path: String, output_path: String) -> bool {
        let Ok(mut input) = File::open(&input_path) else {
            godot_error!("error while opening input file");
            return false;
        };
        let Ok(mut output) = File::create(&output_path) else {
            godot_error!("error while creating output file");
            return false;
        };
        let Ok(mut encoder) = Encoder::new(&mut output, 3) else {
            godot_error!("error while creating encoder");
            return false;
        };
        let mut buf = vec![0u8; CHUNK_SIZE];
        loop {
            let Ok(bytes_read) = input.read(&mut buf) else {
                godot_error!("error while reading input");
                return false;
            };
            if bytes_read == 0 {
                break;
            }
            if encoder.write_all(&buf[..bytes_read]).is_err() {
                godot_error!("error while writing to output");
                return false;
            }
        }
        if encoder.finish().is_err() {
            godot_error!("error while finishing encoder");
            return false;
        }
        true
    }
    #[func]
    fn decompress_file_to_file(&mut self, input_path: String, output_path: String) -> bool {
        let Ok(mut input) = File::open(&input_path) else {
            godot_error!("error while opening input file");
            return false;
        };
        let Ok(mut output) = File::create(&output_path) else {
            godot_error!("error while creating output file");
            return false;
        };
        let Ok(mut decoder) = Decoder::new(&mut input) else {
            godot_error!("error while creating encoder");
            return false;
        };
        let mut buf = vec![0u8; CHUNK_SIZE];
        loop {
            let Ok(bytes_read) = decoder.read(&mut buf) else {
                godot_error!("error while reading input");
                return false;
            };
            if bytes_read == 0 {
                break;
            }
            if output.write_all(&buf[..bytes_read]).is_err() {
                godot_error!("error while writing to output");
                return false;
            }
        }
        true
    }
}
