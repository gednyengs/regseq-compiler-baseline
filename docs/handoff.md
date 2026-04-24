# Regseq Compiler Handoff

The current compiler target is a unified RegseqModel flow:

1. Rust source is analyzed through rustc HIR/MIR.
2. `regseq-driver` constructs a Phase-1 `RegseqModel`.
3. Platform data populates Layer-2 bindings on that same model.
4. The live emit path writes RegseqModel JSON by default.
5. Driver and analysis emitters are accepted CLI emit kinds but are not wired
   into the live flow yet.

Use the RegseqModel JSON as the handoff artifact for validation and backend
work until backend emission is reconnected.

Current validation should focus on:

- whether the source-to-RegseqModel construction path preserves authored facts
- whether platform binding population fills the expected model slots
- whether `--emit model` remains the default user-facing behavior
- whether stale model layers stay out of active crates, tests, and current docs
