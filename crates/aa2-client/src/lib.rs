use godot::prelude::*;

mod game_manager;
mod shop_ui;

struct Aa2Extension;

#[gdextension]
unsafe impl ExtensionLibrary for Aa2Extension {}
