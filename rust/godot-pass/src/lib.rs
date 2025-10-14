use argon2::Argon2;
use argon2::Params;
use godot::classes::Engine;
use godot::prelude::*;

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {
    fn on_level_init(level: InitLevel) {
        if level == InitLevel::Scene {
            Engine::singleton().register_singleton(
                &Argon2Hasher::class_name().to_string_name(),
                &Argon2Hasher::new_alloc(),
            );
        }
    }

    fn on_level_deinit(level: InitLevel) {
        if level == InitLevel::Scene {
            let mut engine = Engine::singleton();
            let singleton_name = &Argon2Hasher::class_name().to_string_name();
            if let Some(my_singleton) = engine.get_singleton(singleton_name) {
                engine.unregister_singleton(singleton_name);
                my_singleton.free();
            } else {
                godot_error!("Failed to get singleton");
            }
        }
    }
}

#[derive(GodotClass)]
#[class(init, base=Object)]
struct Argon2Hasher {
    base: Base<Object>,
}
#[godot_api]
impl Argon2Hasher {
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
