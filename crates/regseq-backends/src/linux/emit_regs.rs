//! Register helpers header emission.
//!
//! Generates register offset definitions and bitfield masks from
//! the portable plan's register obligations and descriptors.

use crate::lowering::plan::DriverLoweringPlan;

use super::c_ast::{generated_file_banner, include_guard_name, CTopLevelItem, CTranslationUnit};
use super::emitter::to_snake_case;
use super::plan::LinuxBackendPlan;

/// Emit the register helpers header with offset defines and field masks.
pub(super) fn emit_register_helpers(
    _plan: &LinuxBackendPlan,
    portable: &DriverLoweringPlan,
) -> String {
    let snake = to_snake_case(&portable.identity.driver_name);
    let driver_name = &portable.identity.driver_name;

    let mut tu = CTranslationUnit::new();

    let filename = format!("{snake}_regs.h");
    let guard = include_guard_name(&filename);

    tu.push(generated_file_banner(&filename, driver_name));
    tu.push(CTopLevelItem::BlankLine);
    tu.push(CTopLevelItem::IncludeGuardOpen(guard.clone()));
    tu.push(CTopLevelItem::BlankLine);

    // PL011 register offsets -- standard ARM PL011 layout
    tu.push(CTopLevelItem::LineComment(
        "Register offsets -- ARM PL011 TRM".into(),
    ));
    emit_pl011_offsets(&mut tu);
    tu.push(CTopLevelItem::BlankLine);

    // Field masks from descriptors
    for desc in &portable.register_obligations.descriptors {
        tu.push(CTopLevelItem::LineComment(format!(
            "Bitfields: {}",
            desc.register_name
        )));
        let reg_prefix = normalize_reg_prefix(&desc.register_name);
        for bf in &desc.bit_fields {
            let mask = compute_mask(bf.offset, bf.width);
            tu.push(CTopLevelItem::Define {
                name: format!("PL011_{}_{}", reg_prefix, bf.name),
                value: Some(format!("{mask:#010x}U")),
            });
        }
        tu.push(CTopLevelItem::BlankLine);
    }

    // LCRH convenience defines for word length, common fields
    tu.push(CTopLevelItem::LineComment("Convenience defines".into()));
    tu.push(CTopLevelItem::Define {
        name: "PL011_LCRH_WLEN_5".into(),
        value: Some("(0x0U << 5)".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: "PL011_LCRH_WLEN_6".into(),
        value: Some("(0x1U << 5)".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: "PL011_LCRH_WLEN_7".into(),
        value: Some("(0x2U << 5)".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: "PL011_LCRH_WLEN_8".into(),
        value: Some("(0x3U << 5)".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: "PL011_LCRH_FEN".into(),
        value: Some("(1U << 4)".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: "PL011_LCRH_STP2".into(),
        value: Some("(1U << 3)".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: "PL011_LCRH_EPS".into(),
        value: Some("(1U << 2)".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: "PL011_LCRH_PEN".into(),
        value: Some("(1U << 1)".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: "PL011_LCRH_BRK".into(),
        value: Some("(1U << 0)".into()),
    });
    tu.push(CTopLevelItem::BlankLine);

    // CR convenience defines
    tu.push(CTopLevelItem::Define {
        name: "PL011_CR_UARTEN".into(),
        value: Some("(1U << 0)".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: "PL011_CR_TXE".into(),
        value: Some("(1U << 8)".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: "PL011_CR_RXE".into(),
        value: Some("(1U << 9)".into()),
    });
    tu.push(CTopLevelItem::BlankLine);

    // IFLS convenience defines
    tu.push(CTopLevelItem::Define {
        name: "PL011_IFLS_RX_HALF".into(),
        value: Some("(0x2U << 3)".into()),
    });
    tu.push(CTopLevelItem::Define {
        name: "PL011_IFLS_TX_EIGHTH".into(),
        value: Some("(0x0U << 0)".into()),
    });
    tu.push(CTopLevelItem::BlankLine);

    tu.push(CTopLevelItem::IncludeGuardClose(guard));

    tu.render()
}

/// Emit standard PL011 register offsets.
fn emit_pl011_offsets(tu: &mut CTranslationUnit) {
    let offsets = [
        ("PL011_DR", 0x000),
        ("PL011_RSR_ECR", 0x004),
        ("PL011_FR", 0x018),
        ("PL011_ILPR", 0x020),
        ("PL011_IBRD", 0x024),
        ("PL011_FBRD", 0x028),
        ("PL011_LCRH", 0x02C),
        ("PL011_CR", 0x030),
        ("PL011_IFLS", 0x034),
        ("PL011_IMSC", 0x038),
        ("PL011_RIS", 0x03C),
        ("PL011_MIS", 0x040),
        ("PL011_ICR", 0x044),
        ("PL011_DMACR", 0x048),
    ];

    for (name, offset) in offsets {
        tu.push(CTopLevelItem::Define {
            name: name.into(),
            value: Some(format!("{offset:#05x}U")),
        });
    }
}

/// Compute a bitmask from offset and width.
fn compute_mask(offset: u32, width: u32) -> u32 {
    ((1u32 << width) - 1) << offset
}

/// Normalize a register module name to a shorter prefix.
///
/// Strips trailing `_REG` if present: `"DR_REG"` -> `"DR"`.
fn normalize_reg_prefix(name: &str) -> &str {
    name.strip_suffix("_REG").unwrap_or(name)
}
