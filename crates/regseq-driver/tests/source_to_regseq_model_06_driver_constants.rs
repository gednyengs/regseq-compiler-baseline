//! Voided pending MonoRust + RegseqModel redesign.
//! See docs/discussions/compiler-refactoring/test-audit.md for the classification rationale
//! and the retirement criterion.
#![cfg(feature = "post-monorust-tests")]

#![feature(rustc_private)]

extern crate rustc_driver;
extern crate rustc_hir;
extern crate rustc_interface;
extern crate rustc_middle;
extern crate rustc_session;

#[path = "support/regseq_model_harness.rs"]
mod harness;

use harness::{
    body_mentions_constant, build_model_with_method_bodies, compile_and_build,
    find_driver_constant, find_driver_impl_block, find_driver_impl_method, resolve_symbol,
};
use regseq_model::ir::LiteralValue;

const SPECIMEN: &str = r#"
#![no_std]

#[rseq::driver(name = "pl011-constants", compatible = ["arm,pl011"], bus = "amba")]
pub struct Pl011Consts;

impl Pl011Consts {
    const DEFAULT_UART_CLOCK_HZ: u32 = 48_000_000;
    const ERROR_INTERRUPT_MASK: u32 = 0x0780;
    const EXPLICIT_ISR_CLEAR_MASK: u32 = 0x0780;

    fn uart_clock_hz(&self) -> u32 {
        Self::DEFAULT_UART_CLOCK_HZ
    }

    fn clear_mask(&self, status: u32) -> u32 {
        let clear_mask = status & Self::EXPLICIT_ISR_CLEAR_MASK;
        clear_mask | Self::ERROR_INTERRUPT_MASK
    }
}
"#;

#[test]
fn driver_impl_constants_survive_as_model_constants() {
    let model = compile_and_build(SPECIMEN, "source_to_regseq_model_06_constants")
        .expect("specimen must build");
    let impl_block = find_driver_impl_block(&model);
    assert_eq!(impl_block.associated_constants.len(), 3);

    let default_clock = find_driver_constant(&model, "DEFAULT_UART_CLOCK_HZ");
    assert_eq!(default_clock.value, LiteralValue::U32(48_000_000));

    let error_mask = find_driver_constant(&model, "ERROR_INTERRUPT_MASK");
    assert_eq!(error_mask.value, LiteralValue::U32(0x0780));

    let explicit_mask = find_driver_constant(&model, "EXPLICIT_ISR_CLEAR_MASK");
    assert_eq!(explicit_mask.value, LiteralValue::U32(0x0780));
}

#[test]
fn migrated_bodies_reference_driver_constants_through_model_ids() {
    let model = build_model_with_method_bodies(
        SPECIMEN,
        "source_to_regseq_model_06_bodies",
        &[
            ("Pl011Consts", "uart_clock_hz"),
            ("Pl011Consts", "clear_mask"),
        ],
    )
    .expect("specimen must build with bodies");

    let default_clock = find_driver_constant(&model, "DEFAULT_UART_CLOCK_HZ");
    let error_mask = find_driver_constant(&model, "ERROR_INTERRUPT_MASK");
    let explicit_mask = find_driver_constant(&model, "EXPLICIT_ISR_CLEAR_MASK");

    let uart_clock_hz = find_driver_impl_method(&model, "uart_clock_hz");
    assert!(
        body_mentions_constant(&uart_clock_hz.body, default_clock.id),
        "uart_clock_hz should carry the DEFAULT_UART_CLOCK_HZ constant by id"
    );

    let clear = find_driver_impl_method(&model, "clear_mask");
    assert!(
        body_mentions_constant(&clear.body, error_mask.id),
        "clear_explicit_interrupts should reference ERROR_INTERRUPT_MASK"
    );
    assert!(
        body_mentions_constant(&clear.body, explicit_mask.id),
        "clear_explicit_interrupts should reference EXPLICIT_ISR_CLEAR_MASK"
    );

    let names: Vec<_> = find_driver_impl_block(&model)
        .associated_constants
        .iter()
        .map(|constant| resolve_symbol(&model, constant.name))
        .collect();
    assert_eq!(
        names,
        vec![
            "DEFAULT_UART_CLOCK_HZ",
            "ERROR_INTERRUPT_MASK",
            "EXPLICIT_ISR_CLEAR_MASK"
        ]
    );
}
