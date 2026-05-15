//! Target context captured with a `RegseqModel`.

use serde::{Deserialize, Serialize};

use crate::ids::StringId;

/// Number of signed integer primitive kinds.
pub const INT_KIND_COUNT: usize = 6;
/// Number of unsigned integer primitive kinds.
pub const UINT_KIND_COUNT: usize = 6;
/// Number of floating-point primitive kinds.
pub const FLOAT_KIND_COUNT: usize = 4;

/// ABI size and alignment in bytes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SizeAlign {
    /// ABI size in bytes.
    pub size: u64,
    /// ABI alignment in bytes.
    pub align: u64,
}

/// Target byte order.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Endian {
    /// Little-endian byte order.
    Little,
    /// Big-endian byte order.
    Big,
}

/// Scalar ABI layouts indexed by primitive kind discriminants.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ScalarLayouts {
    /// Boolean layout.
    pub bool_: SizeAlign,
    /// Unicode scalar layout.
    pub char_: SizeAlign,
    /// Signed integer layouts.
    pub int: [SizeAlign; INT_KIND_COUNT],
    /// Unsigned integer layouts.
    pub uint: [SizeAlign; UINT_KIND_COUNT],
    /// Floating-point layouts.
    pub float: [SizeAlign; FLOAT_KIND_COUNT],
    /// Function pointer layout.
    pub fn_ptr: SizeAlign,
    /// Data pointer layout.
    pub data_ptr: SizeAlign,
}

/// Target properties needed by model consumers.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TargetInfo {
    /// Target triple.
    pub triple: StringId,
    /// Pointer width in bits.
    pub pointer_width: u8,
    /// Target byte order.
    pub endian: Endian,
    /// LLVM data-layout string.
    pub data_layout: StringId,
    /// Primitive scalar layouts.
    pub scalar_layouts: ScalarLayouts,
}

/// Signed integer kind.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntKind {
    /// 8-bit signed integer.
    I8 = 0,
    /// 16-bit signed integer.
    I16 = 1,
    /// 32-bit signed integer.
    I32 = 2,
    /// 64-bit signed integer.
    I64 = 3,
    /// 128-bit signed integer.
    I128 = 4,
    /// Pointer-width signed integer.
    Isize = 5,
}

/// Unsigned integer kind.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum UintKind {
    /// 8-bit unsigned integer.
    U8 = 0,
    /// 16-bit unsigned integer.
    U16 = 1,
    /// 32-bit unsigned integer.
    U32 = 2,
    /// 64-bit unsigned integer.
    U64 = 3,
    /// 128-bit unsigned integer.
    U128 = 4,
    /// Pointer-width unsigned integer.
    Usize = 5,
}

/// Floating-point kind.
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum FloatKind {
    /// 16-bit floating-point value.
    F16 = 0,
    /// 32-bit floating-point value.
    F32 = 1,
    /// 64-bit floating-point value.
    F64 = 2,
    /// 128-bit floating-point value.
    F128 = 3,
}

/// Conservative scalar layouts for constructor defaults.
pub fn default_scalar_layouts(pointer_width: u8) -> ScalarLayouts {
    let ptr = SizeAlign {
        size: u64::from(pointer_width / 8),
        align: u64::from(pointer_width / 8),
    };
    ScalarLayouts {
        bool_: SizeAlign { size: 1, align: 1 },
        char_: SizeAlign { size: 4, align: 4 },
        int: [
            SizeAlign { size: 1, align: 1 },
            SizeAlign { size: 2, align: 2 },
            SizeAlign { size: 4, align: 4 },
            SizeAlign { size: 8, align: 8 },
            SizeAlign {
                size: 16,
                align: 16,
            },
            ptr,
        ],
        uint: [
            SizeAlign { size: 1, align: 1 },
            SizeAlign { size: 2, align: 2 },
            SizeAlign { size: 4, align: 4 },
            SizeAlign { size: 8, align: 8 },
            SizeAlign {
                size: 16,
                align: 16,
            },
            ptr,
        ],
        float: [
            SizeAlign { size: 2, align: 2 },
            SizeAlign { size: 4, align: 4 },
            SizeAlign { size: 8, align: 8 },
            SizeAlign {
                size: 16,
                align: 16,
            },
        ],
        fn_ptr: ptr,
        data_ptr: ptr,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalar_kind_counts_match_discriminants() {
        assert_eq!(IntKind::Isize as usize + 1, INT_KIND_COUNT);
        assert_eq!(UintKind::Usize as usize + 1, UINT_KIND_COUNT);
        assert_eq!(FloatKind::F128 as usize + 1, FLOAT_KIND_COUNT);
    }

    #[test]
    fn size_align_constructs_with_public_fields() {
        let layout = SizeAlign { size: 4, align: 4 };
        assert_eq!(layout.size, 4);
        assert_eq!(layout.align, 4);
    }

    #[test]
    fn target_info_constructs_with_each_endian() {
        let scalar_layouts = default_scalar_layouts(64);
        for endian in [Endian::Little, Endian::Big] {
            let target = TargetInfo {
                triple: StringId::from_u32(1),
                pointer_width: 64,
                endian,
                data_layout: StringId::from_u32(2),
                scalar_layouts: scalar_layouts.clone(),
            };
            assert!(matches!(target.pointer_width, 16 | 32 | 64));
        }
    }
}
