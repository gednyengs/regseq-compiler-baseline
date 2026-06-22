#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

// C06: a `PinCtrl` field with no `#[rseq::pinctrl(id = "…")]` join key
// (PIN-ERR-MISSING-ID, D07 — no field-name fallback).
include!("../../_lifecycle_surface.rs");
include!("../../_pinctrl_surface.rs");

#[rseq::driver(name = "noid", compatible = ["acme,noid"])]
pub struct NoId {
    pins: PinCtrl,
}

impl Lifecycle for NoId {}

pub fn make() -> NoId { loop {} }
