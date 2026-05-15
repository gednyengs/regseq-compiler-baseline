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

newtype_index! {
    /// Identifies an interned string.
    ///
    /// Indexes into the `StringInterner`. `StringId::ZERO` is
    /// reserved for the empty string by interner construction
    /// (pre-interned in `StringInterner::new`).
    pub struct StringId {}
}

impl serde::Serialize for StringId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.as_u32())
    }
}

impl<'de> serde::Deserialize<'de> for StringId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(d)?;
        if raw > Self::MAX_AS_U32 {
            return Err(serde::de::Error::custom(format!(
                "StringId value {raw} exceeds MAX ({})",
                Self::MAX_AS_U32
            )));
        }
        Ok(Self::from_u32(raw))
    }
}

newtype_index! {
    /// Identifies a `SourceFile` in `RegseqModel.source_files`.
    pub struct SourceFileId {}
}

impl SourceFileId {
    /// Sentinel for bridge-synthesized spans (no source location).
    /// Equal to `SourceFileId::ZERO`; the named form is preferred
    /// at use sites for readability. The bridge populates
    /// `source_files[0]` with the synthesized-file sentinel
    /// (`path = "<synthesized>"`, `content: None`,
    /// `line_starts: vec![0]`); spans on bridge-synthesized
    /// Instances use `Span { file: SourceFileId::SYNTHESIZED,
    /// lo: 0, hi: 0 }`.
    pub const SYNTHESIZED: SourceFileId = SourceFileId::ZERO;
}

impl serde::Serialize for SourceFileId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.as_u32())
    }
}

impl<'de> serde::Deserialize<'de> for SourceFileId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(d)?;
        if raw > Self::MAX_AS_U32 {
            return Err(serde::de::Error::custom(format!(
                "SourceFileId value {raw} exceeds MAX ({})",
                Self::MAX_AS_U32
            )));
        }
        Ok(Self::from_u32(raw))
    }
}

newtype_index! {
    /// Identifies a source-level `Item` in `RegseqModel.items`.
    ///
    /// `Item` itself is defined in stage 12 (Rust Items). Stage 1
    /// forward-declares the ID so that `Instance.item: ItemId` and
    /// the `item_def_paths` side table compile against the
    /// foundation crate.
    pub struct ItemId {}
}

impl serde::Serialize for ItemId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.as_u32())
    }
}

impl<'de> serde::Deserialize<'de> for ItemId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(d)?;
        if raw > Self::MAX_AS_U32 {
            return Err(serde::de::Error::custom(format!(
                "ItemId value {raw} exceeds MAX ({})",
                Self::MAX_AS_U32
            )));
        }
        Ok(Self::from_u32(raw))
    }
}

newtype_index! {
    /// Identifies a `Ty` in `RegseqModel.types`.
    ///
    /// `Ty` itself is defined in stage 3 (Type system). Stage 1
    /// forward-declares the ID so that `GenericArg::Type(TyId)`
    /// and the type-bearing `InstanceKind` variants compile.
    pub struct TyId {}
}

impl serde::Serialize for TyId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.as_u32())
    }
}

impl<'de> serde::Deserialize<'de> for TyId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(d)?;
        if raw > Self::MAX_AS_U32 {
            return Err(serde::de::Error::custom(format!(
                "TyId value {raw} exceeds MAX ({})",
                Self::MAX_AS_U32
            )));
        }
        Ok(Self::from_u32(raw))
    }
}

newtype_index! {
    /// Identifies a `Const` in `RegseqModel.consts`.
    ///
    /// `Const` itself is defined in stage 4 (Constants). Stage 1
    /// forward-declares the ID so that `GenericArg::Const(ConstId)`
    /// and the const-bearing `InstanceKind` variants compile.
    pub struct ConstId {}
}

impl serde::Serialize for ConstId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.as_u32())
    }
}

impl<'de> serde::Deserialize<'de> for ConstId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(d)?;
        if raw > Self::MAX_AS_U32 {
            return Err(serde::de::Error::custom(format!(
                "ConstId value {raw} exceeds MAX ({})",
                Self::MAX_AS_U32
            )));
        }
        Ok(Self::from_u32(raw))
    }
}

newtype_index! {
    /// Identifies a function signature `FnSig` in `RegseqModel.sigs`.
    ///
    /// `FnSig` itself is defined in stage 11 (MIR bodies). Stage 1
    /// forward-declares the ID so that
    /// `InstanceKind::Function { sig: FnSigId, .. }` compiles.
    pub struct FnSigId {}
}

impl serde::Serialize for FnSigId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.as_u32())
    }
}

impl<'de> serde::Deserialize<'de> for FnSigId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(d)?;
        if raw > Self::MAX_AS_U32 {
            return Err(serde::de::Error::custom(format!(
                "FnSigId value {raw} exceeds MAX ({})",
                Self::MAX_AS_U32
            )));
        }
        Ok(Self::from_u32(raw))
    }
}

newtype_index! {
    /// Identifies an `Instance` in `RegseqModel.instances`.
    ///
    /// `Instance` is the primary monomorphized unit; defined in
    /// stage 1 alongside this ID.
    pub struct InstanceId {}
}

impl serde::Serialize for InstanceId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.as_u32())
    }
}

impl<'de> serde::Deserialize<'de> for InstanceId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(d)?;
        if raw > Self::MAX_AS_U32 {
            return Err(serde::de::Error::custom(format!(
                "InstanceId value {raw} exceeds MAX ({})",
                Self::MAX_AS_U32
            )));
        }
        Ok(Self::from_u32(raw))
    }
}

newtype_index! {
    /// Identifies a local in a MIR body.
    pub struct LocalId {}
}

impl LocalId {
    /// Return-place local. Equal to `LocalId::ZERO`.
    pub const RETURN_PLACE: LocalId = LocalId::ZERO;
}

impl serde::Serialize for LocalId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.as_u32())
    }
}

impl<'de> serde::Deserialize<'de> for LocalId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(d)?;
        if raw > Self::MAX_AS_U32 {
            return Err(serde::de::Error::custom(format!(
                "LocalId value {raw} exceeds MAX ({})",
                Self::MAX_AS_U32
            )));
        }
        Ok(Self::from_u32(raw))
    }
}

newtype_index! {
    /// Identifies a basic block in a MIR body.
    pub struct BlockId {}
}

impl BlockId {
    /// Entry basic block. Equal to `BlockId::ZERO`.
    pub const START_BLOCK: BlockId = BlockId::ZERO;
}

impl serde::Serialize for BlockId {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.as_u32())
    }
}

impl<'de> serde::Deserialize<'de> for BlockId {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(d)?;
        if raw > Self::MAX_AS_U32 {
            return Err(serde::de::Error::custom(format!(
                "BlockId value {raw} exceeds MAX ({})",
                Self::MAX_AS_U32
            )));
        }
        Ok(Self::from_u32(raw))
    }
}

newtype_index! {
    /// Identifies a variant within an enum.
    pub struct VariantIdx {}
}

impl serde::Serialize for VariantIdx {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_u32(self.as_u32())
    }
}

impl<'de> serde::Deserialize<'de> for VariantIdx {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let raw = u32::deserialize(d)?;
        if raw > Self::MAX_AS_U32 {
            return Err(serde::de::Error::custom(format!(
                "VariantIdx value {raw} exceeds MAX ({})",
                Self::MAX_AS_U32
            )));
        }
        Ok(Self::from_u32(raw))
    }
}

#[cfg(test)]
#[path = "ids_tests.rs"]
mod tests;
