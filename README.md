[![crates.io](https://img.shields.io/crates/v/neuro-sama.svg)](https://crates.io/crates/neuro-sama)
[![docs.rs](https://docs.rs/neuro-sama/badge.svg)](https://docs.rs/neuro-sama)

# neuro_sama, the Rust crate

A Rust crate that implements the [Neuro-sama game
API](https://github.com/VedalAI/neuro-game-sdk). It doesn't handle
WebSocket communications itself, instead, it works with `tungstenite`
messages, which you can handle whichever way you want.

There's a high-level API and a low-level API available. The low-level
API simply defines the API schema, it's available in the `schema`
submodule. The high-level API is hopefully easier and safer to work
with, it's available in the `game` submodule.

Optionally, a `proposals` feature is available that enables proposed
commands that have not yet been accepted or implemented - you can use it
for testing, but the feature is excluded from semver guarantees.

Another feature is `strip-trailing-zeroes`, which strips `.0` from round
floating point numbers, it may be useful for slightly reducing
schema/context size.

## Changelog

- 0.1.0 - initial release
- 0.2.0 - reworked the API a bit to make it easier to work with and more
  semver-compatible, and added handling for the proposed
  `actions/reregister_all` command.
- 0.2.1 - generate a leaner JSON schema that's hopefully less confusing
- 0.3.0 - add a `proposals` feature.
- 0.3.1 - don't require `Arc` for `Api::new`
- 0.4.0 - convert `Api` from a separate object into a sealed trait
- 0.4.1 - interpret whitespace-only messages as null
- 0.4.2 - cleanup action schemas in `register_actions_raw`
- 0.4.3 - add the `strip-trailing-zeroes` feature
- 0.4.4 - fix invalid name for `action/result`
- 0.4.5 - consider `null` schemas equivalent to untyped `{}`
- 0.4.6 - fix `{}` not being valid input for `null` schemas

## License

TL;DR do whatever you want.

Licensed under either the [BSD Zero Clause License](LICENSE-0BSD)
(https://opensource.org/licenses/0BSD), the [Apache 2.0
License](LICENSE-APACHE) (http://www.apache.org/licenses/LICENSE-2.0) or
the [MIT License](LICENSE-MIT) (http://opensource.org/licenses/MIT), at
your choice.
