# regseq-driver-83: Bridge propagates dropped Item/layout fields (pack, link_section, inline_hint, opaque_handle)

## Description

The model `Item` / `Layout` carry fields the backend already emits correctly, but
the bridge **hardcodes** them instead of reading rustc's values, so the data never
reaches the backend. Four such drops, each blocking a §06 codegen-coverage row:

- **`Layout.pack`** hardcoded `None` → `#[repr(packed)]` structs lose their pack.
  The struct then reaches the layout validator with an unaligned field offset and is
  REJECTED (`LayoutMalformed: FieldLayout offset is not aligned`). Blocks **LY-2**.
- **`Item.opaque_handle_eligible`** hardcoded `false` at every ADT-item site. Blocks
  **IT-14**.
- **`Item.link_section`** hardcoded `None` → `#[link_section="..."]` dropped. Blocks
  **IT-8**.
- **`Item.inline_hint`** hardcoded `None`/`InlineHint::None` → `#[inline(always)]` /
  `#[inline(never)]` dropped. Blocks **IT-9**.

The backend emission for all four already exists and is correct (`layout_emit.rs`
packed/`aligned` attrs; `mod.rs:~802-806` `__attribute__((section/always_inline/
noinline))`; the opaque-handle forward-decl) — only the bridge population is missing.

## Input Context

### Code anchors (confirmed at HEAD)

- `crates/regseq-driver/src/bridge/ty.rs:~1601` — `Layout { ..., pack: None, ... }`
  (hardcoded; should read `rustc_layout`'s pack: `repr(packed)` → `Some(1)`,
  `repr(packed(n))` → `Some(n)`).
- `crates/regseq-driver/src/bridge/ty.rs:673` — `opaque_handle_eligible: false`
  (hardcoded; should reflect the eligibility classifier).
- `crates/regseq-driver/src/bridge/` Item construction — `link_section` / `inline_hint`
  hardcoded (the synth sites at `synth/closure.rs:208-209`, `synth/vtable.rs:702-703`,
  `const_.rs:180-181`, `sort.rs:41-42` are CORRECT to be `None` for synthesized items;
  the fix is the **real source-item** Item construction reading
  `tcx.codegen_fn_attrs(def_id).link_section` / `.inline`).

### Validator interaction (LY-2)

- `crates/regseq-model/src/layout_validator.rs:~615-625` — required-align check that
  rejects the packed struct when `pack: None`. With `pack: Some(n)` the field-offset
  alignment requirement must relax accordingly (confirm the validator already honors
  `pack`, else this task includes that).

## Implementation Notes

Read each field from rustc at the real source-item construction sites (not the
synth sites, which stay `None`): `pack` from `rustc_layout`; `link_section` /
`inline_hint` from `tcx.codegen_fn_attrs` (and static attrs); `opaque_handle_eligible`
from the eligibility classifier. Keep the synthesized-instance defaults unchanged.

## Non-Goals

- No backend change (emission already correct).
- Not other `repr` flavors beyond `packed`/`packed(n)`.

## Test Plan

1. Build + workspace green; the bridge/model/backend test suites.
2. `kind=rust` codegen-coverage fixtures (extend `b3_structs` / `b8_item_kinds`):
   a `#[repr(packed)]` struct (LY-2), a `#[link_section]` static (IT-8), an
   `#[inline(always)]`/`#[inline(never)]` fn (IT-9), an opaque-handle struct (IT-14)
   — each compiles-and-runs and the emitted attrs appear in golden.c; re-run
   `cargo run -p codegen-coverage-runner` → LY-2/IT-8/IT-9/IT-14 green, byte-stable.
3. Bridge unit tests asserting each field is populated from rustc.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-driver`, `regseq-model`,
`regseq-backends`, `codegen-coverage-runner`); single-threaded scoped tests; ledger
byte-stable.

## Acceptance Criteria

1. `repr(packed)` no longer rejected; the packed struct's golden.c carries the
   packed/aligned attrs + sizeof/offsetof asserts — LY-2 green.
2. `link_section` / `inline_hint` / `opaque_handle` reach the backend and emit —
   IT-8 / IT-9 / IT-14 green in the ledger.
3. Synthesized-instance defaults unchanged; fmt + clippy clean; scoped tests green;
   ledger byte-stable.
