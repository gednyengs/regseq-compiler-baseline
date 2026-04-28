//! Concrete typed-ID instantiations.
//!
//! Each ID is generated via the `newtype_index!` macro defined in
//! `crate::idx`. Per the stage-1 spec, each ID gets a hand-written
//! `Serialize`/`Deserialize` pair next to its invocation, with
//! on-wire shape `u32 = Idx::index()` and deserialize-time
//! niche-range validation.

use crate::idx::newtype_index;

newtype_index! {
    /// Identifies a crate the model contains items from.
    ///
    /// Indexes into `RegseqModel.crates` (table populated by a
    /// later stage-1 step).
    pub struct CrateId {}
}

impl serde::Serialize for CrateId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.as_u32())
    }
}

impl<'de> serde::Deserialize<'de> for CrateId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(d)?;
        if raw > Self::MAX_AS_U32 {
            return Err(serde::de::Error::custom(format!(
                "CrateId value {raw} exceeds MAX ({})",
                Self::MAX_AS_U32
            )));
        }
        Ok(Self::from_u32(raw))
    }
}
