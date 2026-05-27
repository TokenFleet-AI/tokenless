//! Encoding strategies for the intelligent format router.
//!
//! Three encoders are provided, each optimized for a different JSON shape:
//! - [`toon_hrv`]: Header-Row-Value encoding for uniform object arrays.
//! - [`enhanced_toon`]: Enhanced TOON for schemas with enums, ranges, and constraints.
//! - [`cjson_compact`]: CJSON-style compact encoding for irregular structures.

mod cjson_compact;
mod enhanced_toon;
mod toon_hrv;

pub use cjson_compact::encode as encode_cjson;
pub use enhanced_toon::encode as encode_enhanced;
pub use toon_hrv::encode as encode_toon_hrv;
