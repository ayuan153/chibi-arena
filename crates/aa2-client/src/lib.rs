use godot::prelude::*;

mod game_manager;
mod shop_ui;
mod board_ui;
mod bench_ui;
mod combat_viewer_ui;
mod god_pick_ui;
mod draft_ui;
mod scoreboard_ui;

struct Aa2Extension;

#[gdextension]
unsafe impl ExtensionLibrary for Aa2Extension {}
