//! A crate implementing the [Neuro-sama game API](https://github.com/VedalAI/neuro-game-sdk/).
//!
//! You will have to bring your own IO (i.e. work with the `tungstenite` or `tokio-tungstenite`
//! crates).
//!
//! The easiest option of getting started is looking at the [`game::Game`] trait documentation.
//!
//! You may enable the `"proposals"` feature flag to enable the proposed commands described in
//! [API proposals](https://github.com/VedalAI/neuro-game-sdk/blob/main/API/PROPOSALS.md). This
//! feature is excluded from semver and is allowed to break on minor releases, because the proposed
//! commands are not implemented on Neuro's side.

pub use neuro_sama_derive as derive;
pub mod game;
pub mod schema;
#[doc(hidden)]
pub use schemars;
#[doc(hidden)]
pub use serde;
