//! Target context captured with a `RegseqModel`.

use serde::{Deserialize, Serialize};

use crate::ids::StringId;
use crate::ty::{FLOAT_KIND_COUNT, INT_KIND_COUNT, UINT_KIND_COUNT};

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

/// Target operating system, used by a backend for framework selection.
///
/// Relocated here from `regseq-config` (R5/D8): platform identity is consumed by
/// `regseq-backends` (which depends on `regseq-model` only), so the canonical
/// home is the model. A platform-file os token is validated against this closed
/// vocabulary by the `regseq-l2` bridge (where the raw→typed conversion lives),
/// which keeps `regseq-config` a `serde`-only leaf parser.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TargetOs {
    /// Linux kernel driver backend.
    Linux,
    /// Zephyr RTOS driver backend.
    Zephyr,
}

impl TargetOs {
    /// Comma-separated accepted tokens, for diagnostics.
    pub const EXPECTED: &'static str = "linux, zephyr";

    /// The canonical lowercase token (`"linux"` / `"zephyr"`).
    pub fn token(self) -> &'static str {
        match self {
            TargetOs::Linux => "linux",
            TargetOs::Zephyr => "zephyr",
        }
    }

    /// Parse a platform-file os token (case-insensitive) against the closed
    /// vocabulary; `None` for an unknown token (the caller maps it to a
    /// diagnostic — see `regseq_l2::resolve_platform_identity`).
    pub fn from_token(token: &str) -> Option<TargetOs> {
        match token.to_ascii_lowercase().as_str() {
            "linux" => Some(TargetOs::Linux),
            "zephyr" => Some(TargetOs::Zephyr),
            _ => None,
        }
    }
}

impl core::fmt::Display for TargetOs {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.token())
    }
}

/// Platform identity declared in the `[platform]` table of a platform file — the
/// single source of target identity (D4). Threaded to a backend via
/// `regseq-backends`' `BackendContext` for framework selection; `generic_c`
/// ignores it. All fields are optional: the driver tolerates an absent platform
/// file, and the "os always known" invariant is deferred to whoever first
/// implements real backend selection (D7). Built by the `regseq-l2` bridge from
/// the structurally-parsed raw form (`regseq_config::PlatformIdentityRaw`).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlatformIdentity {
    /// Target OS, validated against [`TargetOs`].
    pub os: Option<TargetOs>,
    /// Board / machine identifier (free-form; e.g. `"qemu-pc"`).
    pub board: Option<String>,
    /// Architecture identifier (free-form; e.g. `"x86_64"`).
    pub arch: Option<String>,
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
    use crate::ty::{FloatKind, IntKind, UintKind};

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
