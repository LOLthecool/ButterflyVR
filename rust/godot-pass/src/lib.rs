use argon2::Argon2;
use argon2::Params;
use godot::prelude::*;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}

/// godot wrapper around an Argon2 hasher.
/// Available as a global scope singleton.
#[derive(GodotClass)]
#[class(init, singleton)]
struct Argon2Hasher {
    base: Base<Object>,
}
#[godot_api]
impl Argon2Hasher {
    /// hashes the given input using Argon2id
    #[func]
    fn hash(
        &mut self,
        memory_usage_mb: u32,
        iterations: u32,
        parallelism: u32,
        input: String,
        salt: String,
        output_length: u32,
    ) -> PackedByteArray {
        let output_length = output_length as usize;
        let hasher = Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            Params::new(
                memory_usage_mb * 1024,
                iterations,
                parallelism,
                Some(output_length),
            )
            .unwrap_or_default(),
        );
        let mut output: Vec<u8> = vec![0; output_length];
        hasher
            .hash_password_into(input.as_bytes(), salt.as_bytes(), &mut output)
            .unwrap();
        output.into()
    }
}
