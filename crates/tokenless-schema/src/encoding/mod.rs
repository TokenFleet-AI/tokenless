//! Encoding strategies for the intelligent format router.
//!
//! Three encoders are provided, each optimized for a different JSON shape:
//! - [`crate::encoding::encode_toon_hrv`]: Header-Row-Value encoding for uniform object arrays.
//! - [`crate::encoding::encode_enhanced`]: Enhanced TOON for schemas with enums, ranges, and constraints.
//! - [`crate::encoding::encode_cjson`]: CJSON-style compact encoding for irregular structures.

mod cjson_compact;
mod enhanced_toon;
mod toon_hrv;

pub use cjson_compact::encode as encode_cjson;
pub use enhanced_toon::encode as encode_enhanced;
pub(crate) use enhanced_toon::is_schema_object;
pub use toon_hrv::encode as encode_toon_hrv;
