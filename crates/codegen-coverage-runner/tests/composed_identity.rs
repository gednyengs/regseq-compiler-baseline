//! The composed-fixture identity invariant (backend-zephyr-c GRILL C02,
//! merge-gate C01.1 material; resolves O2).
//!
//! `pl011-composed/input.rs` (linux) and `pl011-composed-zephyr/input.rs`
//! must be byte-identical after stripping exactly two marked licensed
//! regions per file — (i) `match-transport` (the linux `amba = [...]` rows /
//! the zephyr `compatible = [...]` list), (ii) `deinit` (the linux
//! `on_deinit` impl / an EMPTY zephyr region) — each begin marker citing its
//! ratifying decision. This ports the L3 cited-deviation-whitelist
//! discipline from traces to spec sources: "one spec, two OSes,
//! OS-conditional differences enumerated, cited, and machine-checked."
//!
//! The strip is pure text (`codegen_coverage_runner::identity`) — no
//! bridging, no rustc — so this binary is NOT interner-sensitive and may
//! hold multiple tests. The strip mechanics' own negatives (citation-less
//! marker fails, divergence names the first line, malformed marker shapes)
//! live as unit tests in `src/identity.rs`; this file locks the REAL files.
//!
//! The init-fail variant fixture (D14) is OUTSIDE this contract by design.
//!
//! `#![feature(rustc_private)]` + `extern crate rustc_driver;` are required
//! by every integration binary of this crate (the library transitively links
//! the rustc-backed bridge), even though this test never bridges.

#![feature(rustc_private)]

extern crate rustc_driver;

use std::path::{Path, PathBuf};

use codegen_coverage_runner::identity::{first_divergence, strip_licensed_regions};

fn composed_corpus() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("fixtures/_composed")
}

fn read(fixture: &str) -> String {
    let path = composed_corpus().join(fixture).join("input.rs");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()))
}

#[test]
fn composed_inputs_are_identical_outside_licensed_regions() {
    let linux = read("pl011-composed");
    let zephyr = read("pl011-composed-zephyr");

    let (linux_remainder, linux_regions) =
        strip_licensed_regions(&linux).expect("linux input.rs licensed regions well-formed");
    let (zephyr_remainder, zephyr_regions) =
        strip_licensed_regions(&zephyr).expect("zephyr input.rs licensed regions well-formed");

    // Exactly two regions per file, in C02 order, each carrying its ratified
    // citation (the strip already rejects an EMPTY citation; locking the
    // exact text keeps the citations reviewed changes, like the L3
    // whitelist).
    for (file, regions, match_citation) in [
        (
            "pl011-composed",
            &linux_regions,
            "linux_c AMBA match gate; A-13/B-5 via D01/D04",
        ),
        (
            "pl011-composed-zephyr",
            &zephyr_regions,
            "A-13/B-5 via D01/D04",
        ),
    ] {
        assert_eq!(
            regions.len(),
            2,
            "{file}: exactly two licensed regions (C02), found {}",
            regions.len()
        );
        assert_eq!(
            regions[0].name, "match-transport",
            "{file}: region (i) name"
        );
        assert_eq!(
            regions[0].citation, match_citation,
            "{file}: region (i) citation"
        );
        assert_eq!(regions[1].name, "deinit", "{file}: region (ii) name");
        assert_eq!(
            regions[1].citation, "B-6/C-3 via D06",
            "{file}: region (ii) citation"
        );
    }

    // The byte-identity gate. Failure names the first diverging line (C02).
    if linux_remainder != zephyr_remainder {
        let (line, l, z) = first_divergence(&linux_remainder, &zephyr_remainder)
            .expect("remainders differ but no diverging line found");
        panic!(
            "composed input.rs remainders diverge at stripped line {line}:\n  \
             pl011-composed:        {l}\n  pl011-composed-zephyr: {z}"
        );
    }
}
