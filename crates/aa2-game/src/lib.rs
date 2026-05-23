//! AA2 game logic crate — shared between client and server.
//! Pure logic: no I/O, no networking.

pub mod combat;
pub mod damage;
pub mod draft;
pub mod economy;
pub mod game;
pub mod god;
pub mod matchup;
pub mod player;
pub mod pool;
pub mod scenario;
pub mod shop;

pub use combat::CombatResult;
pub use draft::DraftState;
pub use game::{GameConfig, GameEvent, GamePhase, GameState, COMBAT_TIMEOUT, GOD_PICK_DURATION, GRACE_PERIOD, ROUND1_DURATION, ROUND_DURATION};
pub use matchup::Matchup;
pub use player::PlayerState;
pub use pool::AbilityPool;
pub use scenario::Action;
pub use shop::ShopState;
