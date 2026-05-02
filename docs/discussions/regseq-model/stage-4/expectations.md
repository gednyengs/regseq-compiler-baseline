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

- **Fill `pretty.rs::render_const` stub.** The stage-1 stub at
  `crates/regseq-model/src/pretty.rs` panics with
  `unimplemented!("stage 4 fills render_const")`. Stage 4
  replaces the body with a renderer that produces strings for
  each const variant.

- **Niche optimization for `Option<ConstId>`.** Stage 1's
  `lib.rs` asserts `Option<ConstId>` is 4 bytes. Don't break it.
