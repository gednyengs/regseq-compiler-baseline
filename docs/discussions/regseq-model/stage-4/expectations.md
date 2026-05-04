# Expectations on stage 4 (incoming)

Stage 4 expands the `Const` placeholder
(`crates/regseq-model/src/placeholders.rs`) into the full
constants-and-literals vocabulary.

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

## From stage 1

- **Pin the canonical const-eval result form.** F4-2 instance
  ordering's `ArgsKey` fingerprint includes const arguments
  (`GenericArg::Const(ConstId)`); two builds must produce
  identical fingerprints for equivalent const-evaluated values.
  Stage 1 reserved the rule; stage 4 pins the canonical const
  representation. See the F4-2 section of `stage-1/spec-snapshot.md`.

- **Const types covered.** Per the spec, stage 4 covers scalars,
  byte arrays, aggregate constants (struct / array / enum
  variants), and function pointers as constants.

- **[CLOSED by D-4-K via deferral to stage 14]**
  **Fill `pretty.rs::render_const` stub.** Stage 4 closed this
  inbound by *deferring* the renderer to stage 14 (extending
  the D14 → stage-13 precedent for `render_ty` one stage
  further because `render_const`'s dependency chain is longer).
  Of the 11 `Const` variants: 5 are self-contained scalars
  (`Bool` / `Char` / `Int` / `Uint` / `Float`); 5 depend on
  `render_ty` (stage 13; `ZST` / `Bytes` / `Aggregate` /
  `EnumValue` / `NullPtr`); `Reloc(InstanceRef)` depends
  additionally on `render_def_path(ItemId)` (stage 12) and
  `render_instance` (stage 14). Stage 14 is the latest of
  those dependencies; landing the renderer there ensures all
  transitive dependencies are filled. The stub at
  `crates/regseq-model/src/pretty.rs` now panics with
  `unimplemented!("stage 14 fills render_const (deferred per
  D-4-K)")`. Outbound expectation filed in
  `stage-14/expectations.md` "From stage 4" with the full
  variant-by-variant dependency analysis. Audit shape:
  `discussions.md` D-4-K parent.

- **Niche optimization for `Option<ConstId>`.** Stage 1's
  `lib.rs` asserts `Option<ConstId>` is 4 bytes. Don't break it.
