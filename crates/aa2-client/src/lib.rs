use godot::prelude::*;

struct Aa2Extension;

#[gdextension]
unsafe impl ExtensionLibrary for Aa2Extension {}
