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
//!
//! The optional feature `strip-trailing-zeroes` strips `.0` from round floating point numbers, it
//! may be useful for slightly reducing schema/context size.

pub use neuro_sama_derive as derive;
pub mod game;
pub mod schema;
#[doc(hidden)]
pub use schemars;
#[doc(hidden)]
pub use serde;

#[cfg(not(feature = "strip-trailing-zeroes"))]
fn to_string<T>(value: &T) -> serde_json::Result<String>
where
    T: ?Sized + serde::Serialize,
{
    serde_json::to_string(value)
}

#[cfg(feature = "strip-trailing-zeroes")]
fn to_string<T>(value: &T) -> serde_json::Result<String>
where
    T: ?Sized + serde::Serialize,
{
    struct Formatter;
    impl serde_json::ser::Formatter for Formatter {
        fn write_f32<W>(&mut self, writer: &mut W, value: f32) -> std::io::Result<()>
        where
            W: ?Sized + std::io::Write,
        {
            if value.is_finite() && value == value.trunc() {
                // SAFETY: the checks are done as specified in the docs:
                // - not NaN
                // - finite
                // - representible in the return type
                unsafe {
                    if value >= 0.0 {
                        if value <= u32::MAX as f32 {
                            serde_json::ser::CompactFormatter
                                .write_u32(writer, value.to_int_unchecked())
                        } else if value <= u64::MAX as f32 {
                            serde_json::ser::CompactFormatter
                                .write_u64(writer, value.to_int_unchecked())
                        } else if value <= u128::MAX as f32 {
                            serde_json::ser::CompactFormatter
                                .write_u128(writer, value.to_int_unchecked())
                        } else {
                            serde_json::ser::CompactFormatter.write_f32(writer, value)
                        }
                    } else if value >= i32::MAX as f32 {
                        serde_json::ser::CompactFormatter
                            .write_i32(writer, value.to_int_unchecked())
                    } else if value >= i64::MAX as f32 {
                        serde_json::ser::CompactFormatter
                            .write_i64(writer, value.to_int_unchecked())
                    } else if value >= i128::MAX as f32 {
                        serde_json::ser::CompactFormatter
                            .write_i128(writer, value.to_int_unchecked())
                    } else {
                        serde_json::ser::CompactFormatter.write_f32(writer, value)
                    }
                }
            } else {
                serde_json::ser::CompactFormatter.write_f32(writer, value)
            }
        }
        fn write_f64<W>(&mut self, writer: &mut W, value: f64) -> std::io::Result<()>
        where
            W: ?Sized + std::io::Write,
        {
            if value.is_finite() && value == value.trunc() {
                // SAFETY: the checks are done as specified in the docs:
                // - not NaN
                // - finite
                // - representible in the return type
                unsafe {
                    if value >= 0.0 {
                        if value <= u32::MAX as f64 {
                            serde_json::ser::CompactFormatter
                                .write_u32(writer, value.to_int_unchecked())
                        } else if value <= u64::MAX as f64 {
                            serde_json::ser::CompactFormatter
                                .write_u64(writer, value.to_int_unchecked())
                        } else if value <= u128::MAX as f64 {
                            serde_json::ser::CompactFormatter
                                .write_u128(writer, value.to_int_unchecked())
                        } else {
                            serde_json::ser::CompactFormatter.write_f64(writer, value)
                        }
                    } else if value >= i32::MAX as f64 {
                        serde_json::ser::CompactFormatter
                            .write_i32(writer, value.to_int_unchecked())
                    } else if value >= i64::MAX as f64 {
                        serde_json::ser::CompactFormatter
                            .write_i64(writer, value.to_int_unchecked())
                    } else if value >= i128::MAX as f64 {
                        serde_json::ser::CompactFormatter
                            .write_i128(writer, value.to_int_unchecked())
                    } else {
                        serde_json::ser::CompactFormatter.write_f64(writer, value)
                    }
                }
            } else {
                serde_json::ser::CompactFormatter.write_f64(writer, value)
            }
        }
        fn write_number_str<W>(&mut self, writer: &mut W, value: &str) -> std::io::Result<()>
        where
            W: ?Sized + std::io::Write,
        {
            serde_json::ser::CompactFormatter.write_number_str(
                writer,
                if value.contains('.') {
                    value.trim_end_matches('0').trim_end_matches('.')
                } else {
                    value
                },
            )
        }
    }
    let mut vec = Vec::with_capacity(128);
    let mut ser = serde_json::Serializer::with_formatter(&mut vec, Formatter);
    value.serialize(&mut ser)?;
    // SAFETY: same code as serde_json::to_string
    let string = unsafe {
        // We do not emit invalid UTF-8.
        String::from_utf8_unchecked(vec)
    };
    Ok(string)
}
