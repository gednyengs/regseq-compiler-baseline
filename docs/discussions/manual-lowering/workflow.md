# Workflow

Operational details for running a hand-lowering session. Read
`methodology.md` first.

## 1. Refresh the dump

```
scripts/dump-fixture-mir.sh tests/fixtures/pl011-full-spec
```

Produces `tests/fixtures/pl011-full-spec/dump/`:

- `pl011_full_spec.mir` -- spec MIR. 43 bodies under `^fn ` markers.
- `regseq.mir` -- facade MIR. Intrinsic stubs (recognized by
  `#[rseq::intrinsic("...")]` attribute) plus real helpers like
  `Mmio::modify`.
- `spec.hir.txt` -- post-expansion HIR. Use for type definitions,
  attribute presence, module structure.
- `mono-items.txt` -- 78-line manifest of monomorphized items.
  Tells you which generic instantiations exist and which trait impls
  rustc emitted.

The `dump/` directory is gitignored. Re-run the script whenever the
fixture or the script's flag set changes.

## 2. Identify the body to lower

Source for the fixture: `tests/fixtures/pl011-full-spec/src/`.

The 43 spec bodies are listed in commit `8524e2b` (search the commit
message) or by running:

```
grep "^fn " tests/fixtures/pl011-full-spec/dump/pl011_full_spec.mir
```

Each `^fn` header carries `<impl at src/driver.rs:NN:CC>` which points
at the source location. Keep both windows open: the source for context,
the MIR for the lowering input.

## 3. Capture decisions

Append to `decisions.md` using the entry format documented at the top of
that file. Each entry covers one body and contains:

- Source signature and path:line.
- The hand-lowered C, in a fenced block.
- Numbered list of decisions made during the lowering, each tagged with
  owning layer (MonoRust / RegseqModel / C backend / open).

Trivial uniform decisions (drop StorageLive, drop unwind annotations)
do not need to be re-logged per body. They are captured once in the
"Uniform rules" section of `decisions.md`. Per-body entries record only
the non-uniform decisions.

If hand-lowering surfaces an empirical observation about MIR shape that
generalizes beyond the body at hand, append it to `findings.md`.

## 4. Validate

For the first body, validation is by visual inspection only -- the C
should compile when assembled with the other bodies and a Linux module
shim, but no body-by-body validation harness exists yet.

When enough bodies are lowered to constitute a complete driver, the
compiled C should boot under QEMU and exercise the device. That
validation step is a future work item; the current session-level
expectation is that captured decisions are reviewable, not that the
output runs.

## 5. End-of-session checklist

Before closing a hand-lowering session:

1. Update `README.md`'s "Status" section with what was completed and
   where the next session should pick up.
2. If new empirical findings landed, confirm they are in `findings.md`.
3. If methodology questions arose that were not resolved, append them
   to a "Pending decisions" section in `README.md` so the next session
   can see them.
4. Commit the documentation update with a conventional message
   (`docs: hand-lowering session NN -- <body name>` or similar).

## 6. Adversarial review

For non-trivial decisions (per `methodology.md` Section 4), invoke an
independent agent with the rule document and the entry-in-progress.
The reviewer's role and prompt structure are documented in
`methodology.md` Section 4; the prompt template lives in
`decisions.md` after the format spec.

## 7. When in doubt, ask

The decision-log format and methodology are documents under
construction. Ambiguities are expected. Record the ambiguity rather
than forcing a resolution; the next session (or the user) resolves it.
