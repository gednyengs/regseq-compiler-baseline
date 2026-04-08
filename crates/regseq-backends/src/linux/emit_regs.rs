//! Register helpers header emission.
//!
//! Generates register offset definitions, bitfield masks, and named
//! value defines entirely from plan data. No device-specific literals
//! appear in this module.

use crate::lowering::plan::DriverLoweringPlan;

use super::c_ast::{generated_file_banner, include_guard_name, CTopLevelItem, CTranslationUnit};
use super::emit_semantic::{
    bitfield_mask_define, named_value_define, normalize_desc_prefix, register_offset_define,
};
use super::emitter::to_snake_case;
use super::plan::LinuxBackendPlan;

/// Emit the register helpers header with offset defines and field masks.
///
/// All defines are generated from `plan.register_layout` and
/// `portable.register_obligations.descriptors`. No hardcoded register
/// offsets, named values, or device-specific convenience defines.
pub(super) fn emit_register_helpers(
    plan: &LinuxBackendPlan,
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

    // Emit register offset defines from plan data.
    for block in &plan.register_layout.blocks {
        let reg_prefix = &block.reg_prefix;
        tu.push(CTopLevelItem::LineComment(format!(
            "Register offsets -- {}",
            block.block_name
        )));
        for field in &block.fields {
            tu.push(CTopLevelItem::Define {
                name: register_offset_define(reg_prefix, &field.name),
                value: Some(format!("{:#05x}U", field.byte_offset)),
            });
        }
        tu.push(CTopLevelItem::BlankLine);
    }

    // Emit bitfield masks from plan descriptors.
    let reg_prefix = plan
        .register_layout
        .blocks
        .first()
        .map(|b| b.reg_prefix.as_str())
        .unwrap_or("REG");

    for desc in &plan.register_layout.descriptors {
        let desc_prefix = normalize_desc_prefix(&desc.register_name);
        tu.push(CTopLevelItem::LineComment(format!(
            "Bitfields: {}",
            desc.register_name
        )));
        for bf in &desc.bit_fields {
            let mask = compute_mask(bf.bit_offset, bf.bit_width);
            tu.push(CTopLevelItem::Define {
                name: bitfield_mask_define(reg_prefix, desc_prefix, &bf.name),
                value: Some(format!("{mask:#010x}U")),
            });

            // Emit named values for this bitfield.
            for nv in &bf.named_values {
                tu.push(CTopLevelItem::Define {
                    name: named_value_define(reg_prefix, desc_prefix, &bf.name, &nv.name),
                    value: Some(format!("({}U << {})", nv.value, bf.bit_offset)),
                });
            }
        }
        tu.push(CTopLevelItem::BlankLine);
    }

    tu.push(CTopLevelItem::IncludeGuardClose(guard));

    tu.render()
}

/// Compute a bitmask from offset and width.
fn compute_mask(offset: u32, width: u32) -> u32 {
    ((1u32 << width) - 1) << offset
}
