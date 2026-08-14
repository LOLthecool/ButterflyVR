use godot::prelude::{ExtensionLibrary, gdextension};

struct MyExtension;

#[gdextension]
unsafe impl ExtensionLibrary for MyExtension {}
