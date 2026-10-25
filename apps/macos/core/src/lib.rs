//! aequalis's C interface (`include/aequalis.h`) as a static library, linked into the macOS app.
//! The symbols are the core's own; this crate only makes them a library the Swift code links.

pub use aequalis::ffi::*;
