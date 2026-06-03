# regseq-backends-34: Tighten entry classifier to certified `core::result::Result` identity (Phase-2a)

> Source: Codex soundness review (a **major** finding) + the follow-up review of the
> archived `regseq-backends-33`. Supersedes the archived `regseq-backends-33` (whose
> rendered-string identity check was FRAGILE). Consumes the certified `is_result`
> flag from `regseq-driver-91`. Decision **D08**, contract **C03**, requirement
> **R09**. Memory [[assessment-soundness-pass]].

## Description

`classify_entry_return` accepts **any** niche-encoded two-variant enum shaped like
`Result<(), E>` — it never confirms the return enum's *identity*. The first fix
attempt (archived `-33`) proposed matching `def_path.rendered == "core::result::Result"`,
but `rendered` is a display string, not identity (the model treats same-name /
different-`StableCrateId` crates as distinct), so a look-alike item would still pass.
Use the **compiler-certified `is_result` flag** (`regseq-driver-91`) instead, with the
`Ok`/`Err` variant-name check kept as a defensive guard. A custom
`enum MyResult { Good, Bad(RegseqError) }` returned from `regseq_main` must
**fail loud** (D08), and the adapter's `Ok = variants[0]` / `Err = variants[1]`
assumption must become sound by construction.

## Input Context

### Code anchors (confirm at HEAD)

- `crates/regseq-backends/src/generic_c/mod.rs:406` — `classify_entry_return`: the
  structural niche/two-variant match (comment `:407` "structural, not by name");
  `:432`–`:433` bind `variants[0]` as Ok / `variants[1]` as Err; `:455`–`:456` the
  `is_framework_error_enum` precedent (an item-identity check on the Err payload).
- `model.items[item].is_result` — the certified `Result` flag added by
  `regseq-driver-91` (this task **consumes** it; do not re-derive identity from a
  rendered string).
- `VariantLayout.name` (resolved via `model.strings`) — the `Ok`/`Err` defensive
  variant-name check.
- `crates/regseq-backends/src/generic_c/mod_tests.rs:7030`/`:7152` — the classifier
  test helper `item(m, ItemKind::Enum, "…")`; extend it so positive tests can set
  `is_result == true`, and so a **spoof** test can mint a look-alike (`is_result ==
  false`) item that still *renders* as `core::result::Result`.

## Implementation Notes

- Bind the output enum's `item`; after the existing shape checks, additionally
  require **`model.items[item].is_result`** (certified, not a string match) **AND**
  `variants[0].name == "Ok"` && `variants[1].name == "Err"`.
- Keep the existing checks (Ok payload all-unit; Err payload a single
  `FrameworkType`-marked enum). Anything failing is a fail-loud `Reject` with a clear
  diagnostic.

## Non-Goals

- No change to the adapter emission, errno table, or oracle.
- Not the bridge primitive (`regseq-driver-91`); not `?`/`Try`/`Option`.

## Test Plan

1. **Negative (spoof — the key case):** a look-alike item with `is_result == false`
   that *renders* as `core::result::Result` with variants `Ok`/`Err` → `Reject`.
   Proves identity is by the certified flag, not the display string.
2. **Negative (custom carrier):** `enum MyResult { Good, Bad(RegseqError) }`
   (`is_result == false`) → `Reject`.
3. **Negative (defensive):** `is_result == true` but variant names ≠ `Ok`/`Err` →
   `Reject`.
4. **Positive regression:** a genuine `Result<(), RegseqError>` (`is_result == true`,
   `Ok`/`Err`) → `ErrResult`; unit-void → `UnitVoid`.
5. Existing classifier + golden tests green; goldens byte-identical (**I02**).

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-backends`,
`codegen-coverage-runner`); `RUST_TEST_THREADS=1 cargo test -p regseq-backends
-p codegen-coverage-runner -- --test-threads=1`; gate `pass`; ledger byte-stable.

## Acceptance Criteria

1. `classify_entry_return` accepts **only** an enum with certified `is_result ==
   true` and variants named `Ok`/`Err`; every look-alike — including an
   `is_result == false` item that renders as `core::result::Result`, and custom
   carriers — is `Reject` (**D08/C03/R09**).
2. The adapter's `Ok = variants[0]` / `Err = variants[1]` assumption is sound by
   construction.
3. The spoof, custom-carrier, and wrong-variant-name negatives plus the positive
   regression all exist.
4. Existing goldens byte-identical (**I02**); gate `pass`; fmt + scoped clippy clean.
