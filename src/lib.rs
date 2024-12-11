/// A crate implementing the [Neuro-sama game API](https://github.com/VedalAI/neuro-game-sdk/).
///
/// You will have to bring your own IO (i.e. work with the `tungstenite` or `tokio-tungstenite`
/// crates).
///
/// The easiest option of getting started is looking at the [`game::Game`] trait documentation.

pub use neuro_sama_derive as derive;
pub mod game;
pub mod schema;
#[doc(hidden)]
pub use serde;
#[doc(hidden)]
pub use schemars;
