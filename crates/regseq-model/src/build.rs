//! Build provenance captured with a `RegseqModel`.

use serde::{Deserialize, Serialize};

use crate::ids::StringId;

/// Bridge and toolchain provenance for one extraction.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BuildInfo {
    /// Interned toolchain identifier.
    pub toolchain: StringId,
    /// Deterministic hash of bridge options that affect model shape.
    pub bridge_options_hash: u128,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_info_constructs() {
        let build = BuildInfo {
            toolchain: StringId::from_u32(1),
            bridge_options_hash: 7,
        };
        assert_eq!(build.toolchain, StringId::from_u32(1));
        assert_eq!(build.bridge_options_hash, 7);
    }
}
