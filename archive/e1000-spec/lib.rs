//! Draft Regseq spec for the Intel 82540EM (E1000) gigabit Ethernet
//! controller.
//!
//! Status: DRAFT (documentation under docs/discussions/e1000-full-spec/).
//! This crate is a high-fidelity authoring sketch, not yet a buildable
//! fixture. It mirrors the role that
//! `tests/fixtures/pl011-full-spec/src/lib.rs` plays for the PL011
//! exemplar, but the underlying compiler wrapper does not yet recognize
//! every attribute used here (notably `#[rseq::dma_region]` and the
//! `Network` device-class). The expected migration path is:
//!
//!   1. close the spec across all stages here in draft form
//!   2. promote the closed draft into `tests/fixtures/e1000-full-spec/`
//!      once the compiler-side surfaces (DMA region/streaming, Network
//!      trait, `regseq::dma::*`, `regseq::net::*`) are in place
//!
//! Variant pin: Intel 82540EM (32-bit parallel-PCI, single MSI/INTx,
//! single TX queue, single RX queue). Out by pin: e1000e family, TSO,
//! MSI-X, multi-queue, advanced RSS, SR-IOV. See
//! `../README.md` Scope pin section.
//!
//! Authoritative references the body cites:
//! - Intel 8254x Software Developer's Manual ("82540EM SDM"), revision
//!   3.4, October 2009. Section numbers in doc-comments refer to that
//!   document.
//! - Linux corpus driver `drivers/net/ethernet/intel/e1000/` (legacy
//!   `e1000`, not `e1000e`), file:line references rooted at
//!   `/Users/nyengele/driverbase/os/linux/`.
//! - Zephyr corpus driver `drivers/ethernet/eth_e1000.c`,
//!   `eth_e1000_priv.h`.
//! - FreeBSD corpus `sys/dev/e1000/` scoped to 82540EM-relevant code
//!   paths (corpus-only; not a backend in the paper claim).
//! - ReactOS corpus `drivers/network/dd/e1000/` (corpus-only).

#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub mod regseq;

mod descriptors;
mod driver;
pub mod registers;

pub use descriptors::{LegacyRxDesc, LegacyTxDesc, RX_DESC_SIZE, TX_DESC_SIZE};
pub use driver::E1000FullSpec;
pub use registers::e1000;
