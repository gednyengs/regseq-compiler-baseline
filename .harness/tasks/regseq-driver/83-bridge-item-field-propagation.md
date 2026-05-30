# regseq-driver-83: Bridge propagates dropped Item/layout fields (pack, link_section, inline_hint)

## Description

The model `Item` / `Layout` carry fields the backend already emits, but the bridge
**hardcodes** them at the real source-item construction sites, so the data never
reaches the backend. Three such drops, each blocking a §06 codegen-coverage row:

- **`Layout.pack`** hardcoded `None` → `#[repr(packed)]` structs lose their pack and
  are REJECTED by the layout validator (`LayoutMalformed: FieldLayout offset is not
  aligned`). Blocks **LY-2**. (The validator already honors `pack = Some(n)`, so
  propagating it both un-rejects the struct and feeds the backend's packed emission.)
- **`Item.link_section`** hardcoded `None` → `#[link_section="..."]` dropped. Blocks
  **IT-8** via the **function** path (the backend applies the section attr to
  functions; see Non-Goals re: statics).
- **`Item.inline_hint`** hardcoded `None`/`InlineHint::None` → `#[inline(always)]` /
  `#[inline(never)]` dropped. Blocks **IT-9**.

The backend emission already exists and is correct for these (`layout_emit.rs` packed/
`aligned`; `mod.rs:~799-817` function `__attribute__((section/always_inline/noinline))`).
Only the bridge population is missing.

> `IT-14` (opaque_handle) and static-`link_section` emission were SPLIT OUT — Codex
> review found they need *backend* work (no generic-C opaque-handle path; statics don't
> apply the section attr), not just bridge propagation. IT-14 → `regseq-backends-24`.

## Input Context

### Code anchors (confirmed at HEAD — Codex review)

- `crates/regseq-driver/src/bridge/ty.rs:~1565-1601` — `Layout { ..., pack: None, ... }`
  (read `rustc_layout`'s pack: `repr(packed)` → `Some(1)`, `repr(packed(n))` → `Some(n)`).
- `crates/regseq-driver/src/bridge/ty.rs:~658-673` + `intern_eager_hir_item`
  (`ty.rs:~702-721`) — the **real source-item** Item construction (updates linkage/flags
  today; the place to populate `link_section` / `inline_hint` from
  `tcx.codegen_fn_attrs(def_id)`). The synth sites (`synth/closure.rs:208-209`,
  `synth/vtable.rs:702-703`, `const_.rs:180-181`, `sort.rs:41-42`) stay `None` — correct
  for synthesized instances.
- `crates/regseq-model/src/layout_validator.rs:~615-618` — already honors `pack=Some(n)`
  (confirm; no validator change expected for LY-2).

## Implementation Notes

At the real source-item sites, read `pack` from `rustc_layout`, and `link_section` /
`inline_hint` from `tcx.codegen_fn_attrs`. Leave synthesized-instance defaults at `None`.

## Non-Goals

- No backend change (function-path emission already correct).
- **Static `link_section`** (the backend doesn't apply the section attr to static
  decls/defs, `mod.rs:~1535-1550`/`~1612-1639`) is a SEPARATE backend follow-up — NOT
  required for IT-8, which a `#[link_section]` **function** greens. File a follow-up if
  static placement is needed later.
- `IT-14` (opaque_handle) → `regseq-backends-24`.
- Not `repr` flavors beyond `packed`/`packed(n)`.

## Test Plan

1. Build + workspace green; bridge/model/backend test suites.
2. `kind=rust` codegen-coverage fixtures (extend `b3_structs` / `b8_item_kinds`):
   a `#[repr(packed)]` struct (LY-2), a `#[link_section]` **function** (IT-8), an
   `#[inline(always)]`/`#[inline(never)]` fn (IT-9) — each compiles-and-runs and the
   emitted attrs appear in golden.c; re-run `cargo run -p codegen-coverage-runner` →
   LY-2/IT-8/IT-9 green, byte-stable.
3. Bridge unit tests asserting each field is populated from rustc at the source-item site.

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-driver`, `regseq-model`,
`regseq-backends`, `codegen-coverage-runner`); single-threaded scoped tests; ledger
byte-stable.

## Acceptance Criteria

1. `repr(packed)` no longer rejected; the packed struct's golden.c carries the
   packed/aligned attrs + sizeof/offsetof asserts — LY-2 green.
2. A `#[link_section]` function and an `#[inline(...)]` function emit their attrs —
   IT-8 / IT-9 green in the ledger.
3. Synthesized-instance defaults unchanged; fmt + clippy clean; scoped tests green;
   ledger byte-stable.
