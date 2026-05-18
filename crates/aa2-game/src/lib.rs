//! AA2 game logic crate — shared between client and server.
//! Pure logic: no I/O, no networking.

pub mod damage;
pub mod draft;
pub mod economy;
pub mod game;
pub mod player;
pub mod pool;
pub mod shop;

pub use game::{GameConfig, GamePhase, GameState};
pub use player::PlayerState;
pub use pool::AbilityPool;
pub use shop::ShopState;
