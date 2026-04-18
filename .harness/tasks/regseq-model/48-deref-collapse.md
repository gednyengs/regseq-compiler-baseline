# regseq-model-48: Rule 4 — Deref Collapse for Transports

## Description

Recognize `<Mmio<T> as Deref>::deref` rustc MIR Calls (and
analogous transport types if added later) and collapse them into
transparent access to the inner block. Preserves the transport
receiver's identity through the collapse so downstream
register-metadata resolution (task 51) can trace `&self` back to
the originating transport resource.

**Scope is narrow.** Per spec.md Section 10.2 and
`compiler-knowledge.md` Section 2, the primary register access
path does NOT go through `Deref::deref`: register intrinsics live
on `Mmio<T>` directly, and `Mmio::modify`'s body calls
`self.read()` and `self.write()` (both intrinsics on
`Mmio<Block>`) without dereferencing first. Deref collapse is
therefore for **rare explicit `Deref::deref` uses** in spec
code.

## Input Context

- `docs/discussions/models/regseq-model/ir-reference.md`
  (Section 8 recognition rule 4)
- `docs/discussions/models/regseq-model/spec.md` (Section 8 type
  system, Section 10.2 register intrinsic architecture)
- `docs/discussions/models/regseq-model/execution-plan.md`
  (Phase 5.4 Rule 4 -- explicit note that primary register
  access does NOT go through Deref)
- `crates/regseq-driver/src/construction.rs` (Pass 2 from tasks
  43-47)
- `crates/regseq-driver/src/extract/abstraction_types.rs`
  (`AbstractionTypeRegistry` — `HashMap<DefId, String>` for
  abstraction-annotated types)

## Implementation Notes

### Recognition

At construction setup, resolve `Deref::deref`'s trait-method
`DefId` once via `tcx.lang_items().deref_trait()` → find the
`deref` fn item on that trait. Cache the `DefId`.

When translating a `mir::TerminatorKind::Call` whose callee
resolves (via `Instance::resolve` or direct trait-method DefId
check) to `<T as Deref>::deref` where `T`'s `DefId` is
registered in `ExtractionRegistries.abstraction_types` with a
transport abstraction (currently `Mmio`):

- Drop the Call. The destination temporary (`&Block`) is
  replaced everywhere it is used by a re-projection rooted at
  the original `&Mmio<T>` receiver (add a `Projection::Deref`
  prefix to Places that projected off the deref result).
- Record the transport receiver Place as the identity carrier
  for downstream register-metadata resolution (task 51).

### Limited scope

The collapse only applies when `T`'s `DefId` is registered with
a transport abstraction (`Mmio` today; future transports add
through the same mechanism). `Deref::deref` on any other type
translates as a regular `Call`.

### Detection signal

Use `ExtractionRegistries.abstraction_types` (live `HashMap<DefId,
String>` from Phase 4). If the receiver type's `DefId` is
registered with abstraction `"mmio"` (or a future transport
abstraction), the deref is a transport deref. Otherwise it is
not.

## Non-Goals

- No deref handling for non-transport types.
- No automatic recognition of indirect deref (e.g., through an
  intermediate `Box<Mmio<T>>`).
- No new IR construct -- collapse rewrites Places only.

## Test Plan

Integration tests under `crates/regseq-driver/tests/`:

- Fixture with an explicit `<Mmio<T> as Deref>::deref` Call
  followed by a field projection on the inner block: verify
  the Deref Call is dropped and the resulting Place chains
  correctly back to the transport receiver.
- Fixture with a `Deref::deref` Call on a non-transport type
  (e.g., `Box<u32>`) to verify it is preserved as a regular
  `Call` and not collapsed.
- Fixture with two consecutive `<Mmio<T> as Deref>::deref`
  Calls on the same receiver; verify both are collapsed
  independently.
- Cross-cutting assertion on the constructed PL011 model: zero
  `Call` terminators target `<Mmio<T> as Deref>::deref` anywhere
  in the module.
- `cargo test -p regseq-driver` passes.

## Review

After tests pass, use `codex:rescue` to verify the collapse
detects only registered transport abstractions via live
`DefId` lookup, transport receiver identity is preserved, and
non-transport `Deref::deref` Calls are not affected. Iterate
until approved.

## Quality Gate

```
cargo test -p regseq-driver
cargo fmt --all -- --check
cargo clippy -p regseq-driver -- -D warnings
```

## Acceptance Criteria

- `<Mmio<T> as Deref>::deref` Calls are eliminated from
  constructed bodies; transport receiver identity is preserved
  through Place rewrites.
- Non-transport `Deref::deref` Calls remain as regular `Call`
  terminators.
- Detection uses live `DefId` lookup, not string matching.
- Codex review approved.
