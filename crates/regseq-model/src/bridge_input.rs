//! Bridge-side inputs needed for model validation.

use serde::{Deserialize, Serialize};

use crate::crate_info::StableCrateId;
use crate::model::RegseqModel;

/// Bridge input facts not redundantly stored in the model.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BridgeInput {
    /// Stable crate id of the spec crate the bridge was invoked on.
    pub spec_crate_stable_id: StableCrateId,
}

impl BridgeInput {
    /// Constructs input matching the model's primary crate.
    pub fn matching(model: &RegseqModel) -> Self {
        Self {
            spec_crate_stable_id: model.crates[model.primary_crate].stable_crate_id,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matching_uses_primary_crate_stable_id() {
        let model = RegseqModel::with_primary("spec", StableCrateId(42));
        assert_eq!(
            BridgeInput::matching(&model),
            BridgeInput {
                spec_crate_stable_id: StableCrateId(42)
            }
        );
    }
}
