use godot::prelude::*;

mod game_manager;
pub mod net_client;
mod main_scene;
mod shop_ui;
mod board_ui;
mod bench_ui;
mod combat_viewer_ui;
mod god_pick_ui;
mod draft_ui;
mod scoreboard_ui;
mod endgame_ui;
mod damage_meter_ui;
mod dev_console;
mod loadout_ui;
mod player_list_ui;
mod ui_helpers;

struct Aa2Extension;

#[gdextension]
unsafe impl ExtensionLibrary for Aa2Extension {}
