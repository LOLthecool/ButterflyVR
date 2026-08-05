use std::fs::File;
use std::io::{Read, Write};

use godot::prelude::*;
use zstd::{Decoder, Encoder};

const CHUNK_SIZE: usize = 1024 * 1024;
const COMPRESSION_LEVEL: i32 = 3;

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
        let Ok(mut input) = File::open(&input_path)
            .inspect_err(|e| godot_error!("error while opening input file: {}", e))
        else {
            return false;
        };
        let Ok(mut output) = File::create(&output_path)
            .inspect_err(|e| godot_error!("error while creating output file: {}", e))
        else {
            return false;
        };
        let Ok(mut encoder) = Encoder::new(&mut output, COMPRESSION_LEVEL)
            .inspect_err(|e| godot_error!("error while creating encoder: {}", e))
        else {
            return false;
        };
        let mut buf = vec![0u8; CHUNK_SIZE];
        loop {
            let Ok(bytes_read) = input
                .read(&mut buf)
                .inspect_err(|e| godot_error!("error while reading input: {}", e))
            else {
                return false;
            };
            if bytes_read == 0 {
                break;
            }
            if let Err(e) = encoder.write_all(&buf[..bytes_read]) {
                godot_error!("error while writing to output: {}", e);
                return false;
            }
        }
        if let Err(e) = encoder.finish() {
            godot_error!("error while finishing encoder: {}", e);
            return false;
        }
        true
    }
    #[func]
    fn decompress_file_to_file(&mut self, input_path: String, output_path: String) -> bool {
        let Ok(mut input) = File::open(&input_path)
            .inspect_err(|e| godot_error!("error while opening input file: {}", e))
        else {
            return false;
        };
        let Ok(mut output) = File::create(&output_path)
            .inspect_err(|e| godot_error!("error while creating output file: {}", e))
        else {
            return false;
        };
        let Ok(mut decoder) = Decoder::new(&mut input)
            .inspect_err(|e| godot_error!("error while creating decoder: {}", e))
        else {
            return false;
        };
        let mut buf = vec![0u8; CHUNK_SIZE];
        loop {
            let Ok(bytes_read) = decoder
                .read(&mut buf)
                .inspect_err(|e| godot_error!("error while reading input: {}", e))
            else {
                return false;
            };
            if bytes_read == 0 {
                break;
            }
            if let Err(e) = output.write_all(&buf[..bytes_read]) {
                godot_error!("error while writing to output: {}", e);
                return false;
            }
        }
        true
    }
}
