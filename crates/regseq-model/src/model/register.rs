//! Register descriptor and register block model types.
//!
//! Register descriptors (see spec.md Section 3.5) define bitfield layouts
//! for individual registers. Register blocks (see spec.md Section 3.6)
//! define ordered memory-mapped register layouts that reference descriptors.
//!
//! Descriptors are standalone, reusable items; the same descriptor may be
//! referenced by multiple register blocks. The descriptor-block reference
//! relationship forms a DAG.

use serde::{Deserialize, Serialize};

use crate::identity::{RegisterBlockId, RegisterDescriptorId, Symbol};

/// A bitfield definition for an individual register.
///
/// Register descriptors are standalone, reusable items -- they are not
/// owned by any single register block and may be referenced by multiple
/// blocks. See spec.md Section 3.5.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RegisterDescriptor {
    /// The descriptor's declaration identifier.
    pub id: RegisterDescriptorId,
    /// The descriptor's identifier as an interned symbol (e.g., `DR_REG`).
    pub name: Symbol,
    /// Register width in bits (e.g., 32 from `u32`).
    pub access_width: u32,
    /// Bitfield definitions within the register.
    pub fields: Vec<BitfieldDef>,
}

/// A single bitfield within a register descriptor.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct BitfieldDef {
    /// Field identifier (e.g., `DATA`, `OE`).
    pub name: Symbol,
    /// Bit offset within the register.
    pub offset: u32,
    /// Number of bits.
    pub width: u32,
    /// Optional named constants for this bitfield.
    pub named_values: Vec<NamedValue>,
}

/// A named constant value for a bitfield (e.g., `ENABLED = 1`).
///
/// `value` is `u64` to accommodate registers up to 64 bits.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct NamedValue {
    /// Value identifier (e.g., `ENABLED`, `DISABLED`).
    pub name: Symbol,
    /// The numeric value.
    pub value: u64,
}

/// A memory-mapped register layout definition.
///
/// Register blocks reference register descriptors but do not own them;
/// the same descriptor may be referenced by multiple blocks. See
/// spec.md Section 3.6.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RegisterBlock {
    /// The block's declaration identifier.
    pub id: RegisterBlockId,
    /// The block's identifier (e.g., `Pl011Registers`).
    pub name: Symbol,
    /// Block-level properties from `#[rseq::register_block(..)]`.
    pub block_attributes: RegisterBlockAttributes,
    /// Ordered list of register fields in the block layout.
    pub fields: Vec<RegisterBlockField>,
}

/// Block-level properties from `#[rseq::register_block(..)]`.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RegisterBlockAttributes {
    /// Byte order (e.g., `little`, `big`).
    pub endian: Symbol,
    /// Default access width in bits (e.g., 32).
    pub access_width: u32,
}

/// A field in a register block layout.
///
/// `descriptor` is `None` for `Reserved` fields. `byte_offset` is
/// derived from field ordering and access width during extraction.
/// `array_count` is `Some` for reserved gaps expressed as arrays
/// (e.g., `[Reserved<u32>; 4]`), and `None` for single fields.
#[derive(Clone, Eq, PartialEq, Debug)]
pub struct RegisterBlockField {
    /// Field identifier (e.g., `dr`, `cr`).
    pub name: Symbol,
    /// Access permission for this field.
    pub access_kind: AccessKind,
    /// Reference to a register descriptor, if not `Reserved`.
    pub descriptor: Option<RegisterDescriptorId>,
    /// Byte offset within the block.
    pub byte_offset: u32,
    /// For reserved gaps expressed as arrays, the element count.
    /// `None` for single fields.
    pub array_count: Option<usize>,
}

/// Access kind for a register block field.
#[derive(Copy, Clone, Eq, PartialEq, Hash, Debug, Serialize, Deserialize)]
#[serde(tag = "kind", content = "data")]
pub enum AccessKind {
    /// Readable and writable.
    ReadWrite,
    /// Readable only.
    ReadOnly,
    /// Writable only.
    WriteOnly,
    /// Reserved gap. No descriptor is attached.
    Reserved,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::{DeclId, SymbolInterner};

    fn sym(interner: &mut SymbolInterner, s: &str) -> Symbol {
        interner.intern(s)
    }

    // -- RegisterDescriptor construction ---------------------------------

    #[test]
    fn constructs_register_descriptor_with_three_fields_including_named_values() {
        let mut interner = SymbolInterner::new();
        let desc_name = sym(&mut interner, "DR_REG");
        let f0 = sym(&mut interner, "DATA");
        let f1 = sym(&mut interner, "OE");
        let f2 = sym(&mut interner, "BE");
        let nv_enabled = sym(&mut interner, "ENABLED");
        let nv_disabled = sym(&mut interner, "DISABLED");

        let desc = RegisterDescriptor {
            id: RegisterDescriptorId::from(DeclId::new(1)),
            name: desc_name,
            access_width: 32,
            fields: vec![
                BitfieldDef {
                    name: f0,
                    offset: 0,
                    width: 8,
                    named_values: vec![],
                },
                BitfieldDef {
                    name: f1,
                    offset: 11,
                    width: 1,
                    named_values: vec![
                        NamedValue {
                            name: nv_enabled,
                            value: 1,
                        },
                        NamedValue {
                            name: nv_disabled,
                            value: 0,
                        },
                    ],
                },
                BitfieldDef {
                    name: f2,
                    offset: 10,
                    width: 1,
                    named_values: vec![],
                },
            ],
        };

        assert_eq!(desc.access_width, 32);
        assert_eq!(desc.fields.len(), 3);
        assert_eq!(desc.fields[0].named_values.len(), 0);
        assert_eq!(desc.fields[1].named_values.len(), 2);
        assert_eq!(desc.fields[1].named_values[0].value, 1);
        assert_eq!(desc.fields[1].named_values[1].value, 0);
    }

    // -- RegisterBlock construction --------------------------------------

    #[test]
    fn constructs_register_block_with_readwrite_reserved_and_readonly_fields() {
        let mut interner = SymbolInterner::new();
        let block_name = sym(&mut interner, "Pl011Registers");
        let endian = sym(&mut interner, "little");
        let dr_field = sym(&mut interner, "dr");
        let rsvd_field = sym(&mut interner, "_rsvd");
        let fr_field = sym(&mut interner, "fr");

        let dr_desc_id = RegisterDescriptorId::from(DeclId::new(1));
        let fr_desc_id = RegisterDescriptorId::from(DeclId::new(2));

        let block = RegisterBlock {
            id: RegisterBlockId::from(DeclId::new(3)),
            name: block_name,
            block_attributes: RegisterBlockAttributes {
                endian,
                access_width: 32,
            },
            fields: vec![
                RegisterBlockField {
                    name: dr_field,
                    access_kind: AccessKind::ReadWrite,
                    descriptor: Some(dr_desc_id),
                    byte_offset: 0x00,
                    array_count: None,
                },
                RegisterBlockField {
                    name: rsvd_field,
                    access_kind: AccessKind::Reserved,
                    descriptor: None,
                    byte_offset: 0x04,
                    array_count: Some(4),
                },
                RegisterBlockField {
                    name: fr_field,
                    access_kind: AccessKind::ReadOnly,
                    descriptor: Some(fr_desc_id),
                    byte_offset: 0x18,
                    array_count: None,
                },
            ],
        };

        assert_eq!(block.block_attributes.access_width, 32);
        assert_eq!(block.fields.len(), 3);

        assert_eq!(block.fields[0].access_kind, AccessKind::ReadWrite);
        assert_eq!(block.fields[0].descriptor, Some(dr_desc_id));
        assert_eq!(block.fields[0].byte_offset, 0x00);
        assert_eq!(block.fields[0].array_count, None);

        assert_eq!(block.fields[1].access_kind, AccessKind::Reserved);
        assert!(
            block.fields[1].descriptor.is_none(),
            "reserved fields must not carry a descriptor"
        );
        assert_eq!(block.fields[1].byte_offset, 0x04);
        assert_eq!(block.fields[1].array_count, Some(4));

        assert_eq!(block.fields[2].access_kind, AccessKind::ReadOnly);
        assert_eq!(block.fields[2].descriptor, Some(fr_desc_id));
        assert_eq!(block.fields[2].byte_offset, 0x18);
    }

    // -- AccessKind serde round-trip -------------------------------------

    #[test]
    fn access_kind_round_trips_through_json_for_every_variant() {
        for variant in [
            AccessKind::ReadWrite,
            AccessKind::ReadOnly,
            AccessKind::WriteOnly,
            AccessKind::Reserved,
        ] {
            let json = serde_json::to_string(&variant).expect("serialize");
            let back: AccessKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(variant, back, "round-trip mismatch for {variant:?}");
        }
    }

    #[test]
    fn access_kind_uses_adjacently_tagged_format() {
        let json = serde_json::to_string(&AccessKind::ReadOnly).unwrap();
        assert!(
            json.contains("\"kind\""),
            "adjacently tagged variants must emit a 'kind' field: {json}",
        );
    }

    #[test]
    fn access_kind_variants_are_distinguishable_in_json() {
        let rw = serde_json::to_string(&AccessKind::ReadWrite).unwrap();
        let ro = serde_json::to_string(&AccessKind::ReadOnly).unwrap();
        let wo = serde_json::to_string(&AccessKind::WriteOnly).unwrap();
        let res = serde_json::to_string(&AccessKind::Reserved).unwrap();
        assert_ne!(rw, ro);
        assert_ne!(rw, wo);
        assert_ne!(rw, res);
        assert_ne!(ro, wo);
        assert_ne!(ro, res);
        assert_ne!(wo, res);
    }

    // -- Descriptor sharing (DAG property) -------------------------------

    #[test]
    fn two_blocks_may_reference_the_same_descriptor() {
        let mut interner = SymbolInterner::new();
        let shared_desc = RegisterDescriptorId::from(DeclId::new(10));
        let endian = sym(&mut interner, "little");

        let mk_block = |id: u32, name: Symbol| RegisterBlock {
            id: RegisterBlockId::from(DeclId::new(id)),
            name,
            block_attributes: RegisterBlockAttributes {
                endian,
                access_width: 32,
            },
            fields: vec![RegisterBlockField {
                name,
                access_kind: AccessKind::ReadWrite,
                descriptor: Some(shared_desc),
                byte_offset: 0,
                array_count: None,
            }],
        };

        let a_name = sym(&mut interner, "BlockA");
        let b_name = sym(&mut interner, "BlockB");
        let a = mk_block(20, a_name);
        let b = mk_block(21, b_name);

        assert_eq!(a.fields[0].descriptor, Some(shared_desc));
        assert_eq!(b.fields[0].descriptor, Some(shared_desc));
    }
}
