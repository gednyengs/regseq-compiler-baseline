# regseq-model-73: VTable-family Items, SyntheticItemKind, PrincipalTraitKey, synthetic DefPath segments + DefPathHash construction (P1a-DS-08b)

## Description

Implement the vtable-family synthesised-Item data shapes (per
`spec.md` §13.2), the full closed `SyntheticItemKind` enum that
discriminates all six synthetic Item kinds (per `spec.md` §13.3),
the `PrincipalTraitKey` + `ProjectionBindingsKey` + `ContentHash`
identity-carrier types, the synthetic `<regseq_synth>`-prefixed
`DefPath.segments` encoding (§13.4), and the synthetic
`DefPathHash` construction algorithm including the reserved
`StableCrateId(0)` synthetic-domain marker.

This task closes the synthesised-Item data-shape surface alongside
the closure-family discriminator variants (regseq-model-72): the
vtable family (VTableType / VTableInstance / VTableMethodShim /
VTableDropShim), the const-memory family
(`ConstMemoryStatic`), the `ProjectionBindingsKey` /
`PrincipalTraitKey` / `ContentHash` keying types, the
domain-separated `<regseq_synth>` DefPath segment grammar, and the
synthetic-`DefPathHash` construction (`StableCrateId(0)` reserved
marker; SHA-256 truncated `local_hash`). The synthesised vtable
*layout* algorithm (§13.2) and the bridge-side synthesis pipeline
that *populates* these Items at construction time are out of
scope; this task pins data shape + keying primitives.

## Input Context

- `docs/discussions/regseq-model/spec.md` §13.2 (Vtable family;
  `spec.md:4589-4814`)
- `docs/discussions/regseq-model/spec.md` §13.3 (`SyntheticItemKind`
  + `PrincipalTraitKey`; `spec.md:4815-5030`)
- `docs/discussions/regseq-model/spec.md` §13.4 (Synthetic
  `DefPath` segments + `DefPathHash` construction;
  `spec.md:5031-5217`)
- `docs/discussions/regseq-model/spec.md` §3.1 invariant 2
  (`item_def_paths` uniqueness + ClosureCaptures carve-out)
- `docs/discussions/regseq-model/spec.md` §3.1 invariant 20
  (`StableCrateId(0)` synthetic-domain enforcement)
- `docs/discussions/regseq-model/spec.md` §7 (DefPath / DefPathHash
  schema; `spec.md:2013-2110` defines the `(StableCrateId, u64)`
  byte form)
- `docs/discussions/regseq-model/spec.md` §12.3 (`ArgsKey` canonical
  byte encoding -- the hash family `PrincipalTraitKey` and
  `ContentHash` reuse)
- `crates/regseq-model/src/synthetic.rs` (closure-family variants
  shipped by regseq-model-72; this task amends to add the vtable
  + const-memory variants)
- `crates/regseq-model/src/ids.rs` / `crates/regseq-model/src/def_path.rs`
  (`DefPathHash`, `StableCrateId`, `DefPath`, `DefPathSegment`)
- `.harness/spec-batches/regseqmodel-phase-1a.md` (parent brief;
  P1a-DS-08b row)

## Implementation Notes

Extend `crates/regseq-model/src/synthetic.rs` (created in
regseq-model-72) with the vtable + const-memory `SyntheticItemKind`
variants, the keying types (`PrincipalTraitKey`,
`ProjectionBindingsKey`, `ContentHash`), and a new sibling module
`crates/regseq-model/src/synthetic_def_path.rs` for the synthetic
DefPath segment encoding + synthetic `DefPathHash` construction
algorithm (re-export from `lib.rs`).

### Public API -- SyntheticItemKind (full closed enum)

Per `spec.md:4861-4905`, the full closed enum (closure variants
already shipped by regseq-model-72; this task adds the rest):

```rust
pub enum SyntheticItemKind {
    /// Source-level item from rustc; `item_def_paths[id]` is
    /// `tcx.def_path_hash(did)` for the originating `DefId`.
    None,

    // Closure family (shipped by regseq-model-72; reproduced here
    // for the closed-enum context).
    ClosureCaptures  { closure: DefPathHash },
    ClosureCall      { closure: DefPathHash, call_kind: ClosureCallKind },
    ClosureDropGlue  { closure: DefPathHash },

    // Vtable family (§13.2). Identity does NOT include any
    // "triggering crate" -- vtable Items are deduped by their
    // semantic identity (PrincipalTraitKey, plus self_ty and
    // vtable_entry_index/method_def_path where relevant), not
    // by discovery order.
    VTableType       { principal: PrincipalTraitKey },
    VTableInstance   { principal: PrincipalTraitKey, self_ty: ArgsKey },
    VTableMethodShim { principal: PrincipalTraitKey, self_ty: ArgsKey, vtable_entry_index: u32, method_def_path: DefPathHash },
    VTableDropShim   { principal: PrincipalTraitKey, self_ty: ArgsKey },

    // Const-memory family (§13.4, §9.7).
    ConstMemoryStatic { content_hash: ContentHash },
}
```

### Public API -- keying types

Per `spec.md:4821-4859`:

```rust
pub struct PrincipalTraitKey {
    pub trait_path:          DefPathHash,
    pub trait_args:          ArgsKey,
    pub projection_bindings: ProjectionBindingsKey,
}

pub struct ProjectionBindingsKey(pub u128);    // 128-bit content hash

pub struct ContentHash(pub u128);              // 128-bit content hash
```

### Public API -- synthetic DefPath segment construction

Per `spec.md:5051-5095` (the per-family canonical segment
templates), provide a constructor function returning
`Vec<DefPathSegment>` (or the project's equivalent segment
container) for the seven synthetic kinds whose Item identity is
bridge-synthesised (the `ClosureCaptures` carve-out is excluded;
it uses rustc's source-derived DefPath verbatim per
`spec.md:5042-5045`):

```rust
/// Build the canonical synthetic `DefPath.segments` for the
/// given synthesised-Item kind. Returns segments using
/// `DefPathData::TypeNs(StringId)` per spec.md:5051-5095.
/// The first segment is always `TypeNs("<regseq_synth>")`.
///
/// Panics if `kind == SyntheticItemKind::None` or
/// `kind == SyntheticItemKind::ClosureCaptures { .. }` --
/// neither uses synthetic segments per §13.4 Scope.
pub fn synthetic_def_path_segments(
    kind: &SyntheticItemKind,
    strings: &mut StringInterner,
) -> Vec<DefPathSegment>;
```

### Public API -- synthetic DefPathHash construction

Per `spec.md:5126-5184`, expose the synthetic-`DefPathHash`
constructor:

```rust
/// Construct the canonical synthetic `DefPathHash` for a
/// bridge-synthesised Item per spec.md:5126-5184. Yields
/// `DefPathHash { stable_crate_id: StableCrateId(0),
/// local_hash: <sha256-truncated-u64> }`.
///
/// Panics if `kind == SyntheticItemKind::None` or
/// `kind == SyntheticItemKind::ClosureCaptures { .. }` --
/// neither uses synthetic hashing per §13.4 Scope (the
/// captures Item carries rustc's source-derived hash).
pub fn synthetic_def_path_hash(kind: &SyntheticItemKind) -> DefPathHash;
```

### Conventions

- **`SyntheticItemKind` is closed and append-only.** New
  bridge-synthesis families extend with new variants append-only
  (per `spec.md:5026-5029`); existing variants and their payload
  shapes remain stable.
- **`PrincipalTraitKey` rationale.** The trait identity at the
  trait-object boundary is `(trait_path, trait_args,
  projection_bindings)`; collapsing to just `(trait_path,
  projection_bindings)` would conflate `dyn Register<u32>` with
  `dyn Register<u64>` (per `spec.md:4983-4998`).
- **`ProjectionBindingsKey` is a distinct type from `ArgsKey`**
  even though both are 128-bit content fingerprints, because
  projection bindings are *named* `(assoc_item, value)` pairs
  rather than positional args (per `spec.md:4836-4843`). The
  canonical input grammar differs (per `spec.md:4914-4929`).
- **`ContentHash` canonical input** is the canonical
  `Const::Bytes(Vec<Byte>)` byte-encoding per `spec.md:4957-4965`.
- **`<regseq_synth>` reserved domain-separator prefix.** First
  segment of every synthetic DefPath is
  `TypeNs(strings.intern("<regseq_synth>"))` per
  `spec.md:5054-5060`. This guarantees synthetic Items never alias
  with source-derived items even if a future toolchain bump adds
  a crate named `regseq_synth`.
- **`StableCrateId(0)` is the reserved synthetic-crate marker**
  per `spec.md:5137-5147`. Construction invariant 20 (§3.1)
  requires the bridge to verify no real crate has
  `stable_crate_id == 0`; rejection diagnostic
  `BridgeError::StableCrateIdZeroCollision` is bridge-side
  (P1a-BR-01 / P1a-VAL-03 territory).
- **Synthetic `DefPathHash.local_hash` construction** per
  `spec.md:5128-5147`: `u64` from leading 8 bytes (big-endian)
  of `SHA-256(b"regseq-synthetic-item-v1\0" ++ u8 variant_tag
  ++ canonical payload)`. Per-family canonical payload bytes per
  `spec.md:5151-5178`.
- **The `ClosureCaptures` carve-out** (`spec.md:5042-5048`):
  this synthetic kind is *excluded* from synthetic segment /
  hash construction. The discriminator is a marker only; the
  Item's `def_path` and `item_def_paths[id]` come from rustc.
  The synthetic constructors above must reject it (panic in
  debug + return `Err` or panic in release per project
  convention -- recommend `panic!` since it is a programming
  error, not data).
- **`SyntheticItemKind::None` discriminant** marks
  source-derived Items (`spec.md:4862-4864`); its presence is
  what allows the side-table to be a uniform `IndexVec<ItemId,
  SyntheticItemKind>` without an outer `Option`.
- **Serde**: `SyntheticItemKind`, `PrincipalTraitKey`,
  `ProjectionBindingsKey`, `ContentHash` all contain only
  `DefPathHash` / `ArgsKey` / `u128` payloads (no `Symbol` or
  `StringId`), so serde derives are admissible. Use
  `#[derive(Serialize, Deserialize)]`. The synthetic-DefPath
  segment constructor produces `Vec<DefPathSegment>` whose
  segments contain `StringId`s (deferred from serde per the
  brief's cross-cutting note); the function itself is
  serialisation-neutral.
- **Derive `Clone, Debug, PartialEq, Eq, Hash`** on
  `SyntheticItemKind`, `PrincipalTraitKey`,
  `ProjectionBindingsKey`, `ContentHash`. `PartialOrd` / `Ord`
  on `PrincipalTraitKey` is reachable via the embedded byte
  forms; defer to a follow-up if the F4 sort comparator
  (P1a-VAL-02) needs it.
- **Collision model framing.** Both `ProjectionBindingsKey` and
  `ContentHash` carry the same "stable content fingerprint --
  not proof of equality" framing as `ArgsKey` per
  `spec.md:4942-4949` and `spec.md:4976-4981`. Document this in
  the type-level doc comment.

## Non-Goals

- Closure-family `SyntheticItemKind` variants (`ClosureCaptures`,
  `ClosureCall`, `ClosureDropGlue`) and `ClosureCallKind` ship
  in regseq-model-72 (P1a-DS-08a); this task only adds the
  remaining vtable + const-memory variants.
- The synthesised-vtable *layout* algorithm (`spec.md:4680-4742`)
  is bridge-side compute (P1a-BR-04). This task does not
  implement the layout algorithm; it only commits the
  `SyntheticItemKind::VTableType` discriminator.
- Bridge-side vtable / closure / drop-glue / const-memory
  synthesis (Phase-2 working caches; Phase-5 re-key to final
  ItemId; `RegseqModel.vtables` `HashMap<PrincipalTraitKey,
  ItemId>` population) is P1a-BR-04 / P1a-BR-03a territory.
- The `SyntheticItemKind::ClosureCaptures` invariant 2 carve-out
  enforcement (`item_def_paths` uniqueness with the
  source-derived exception) lands in P1a-VAL-02.
- The `StableCrateId(0)` non-collision check per invariant 20
  (`BridgeError::StableCrateIdZeroCollision`) is bridge-side
  (P1a-BR-01) and validator-side (P1a-VAL-03).
- The Phase-C `vtables` rebuild + duplicate rejection
  (`spec.md:4795-4814`) is validator-side (P1a-VAL-02).
- Receiver-erasure rules (`spec.md:4607-4622`) and method-shim
  ABI rewrites (`spec.md:4648-4678`) are bridge-side (P1a-BR-04).

## Test Plan

**Test organization**: `unit-in-src` -- tests live in
`crates/regseq-model/src/synthetic.rs` and
`crates/regseq-model/src/synthetic_def_path.rs`'s
`#[cfg(test)] mod tests` blocks.

Coverage requirements (every public type + function exercised at
least once per `SESSION.md` Testing Requirements):

1. **`SyntheticItemKind::None` construction.** Construct
   `SyntheticItemKind::None`; assert clone equality.
2. **`SyntheticItemKind::VTableType` construction.** Construct
   with a fixture `PrincipalTraitKey`; assert equality with a
   structurally identical second value; assert inequality when
   `principal.trait_path` differs.
3. **`SyntheticItemKind::VTableInstance` construction.**
   Construct with fixture `(principal, self_ty)`; assert
   distinctness from `VTableType { principal }` even when
   `principal` is identical.
4. **`SyntheticItemKind::VTableMethodShim` construction.**
   Construct two values that share `(principal, self_ty,
   method_def_path)` but differ on `vtable_entry_index`; assert
   inequality.
5. **`SyntheticItemKind::VTableDropShim` construction.**
   Construct with fixture `(principal, self_ty)`; assert
   distinctness from `VTableInstance { principal, self_ty }`
   sharing the same payload.
6. **`SyntheticItemKind::ConstMemoryStatic` construction.**
   Construct with fixture `ContentHash(0xdead_beef_...)`; assert
   distinctness from a `ContentHash` differing by 1 bit.
7. **All variants pairwise distinct.** Build one value per
   `SyntheticItemKind` variant (sharing whatever payload makes
   sense) and assert all pairs are inequal -- ensures the enum's
   variant tag is part of equality.
8. **`PrincipalTraitKey` equality + Hash.** Two values with
   identical `(trait_path, trait_args, projection_bindings)` are
   `==` and hash identically; differing on any one field
   distinguishes them.
9. **`ProjectionBindingsKey(u128)` round-trip.** Construct from
   a known `u128`; assert `.0` returns the same value; clone +
   equality work.
10. **`ContentHash(u128)` round-trip.** Same as item 9 for
    `ContentHash`.
11. **Serde round-trip for `SyntheticItemKind` variants.** Each
    variant survives `serde_json::to_string` / `from_str` round
    trips with `==`. Cover all 9 variants (None + 3 closure + 4
    vtable + 1 const-memory).
12. **Serde round-trip for `PrincipalTraitKey`,
    `ProjectionBindingsKey`, `ContentHash`** -- each round-trips
    with `==`.
13. **`synthetic_def_path_segments` -- `<regseq_synth>` first
    segment.** For each non-excluded variant (vtable family +
    `ClosureCall`/`ClosureDropGlue` + `ConstMemoryStatic`), the
    first returned segment is `TypeNs(string_id)` resolving to
    the literal `"<regseq_synth>"`.
14. **`synthetic_def_path_segments` -- per-family segment
    counts.** For each non-excluded variant, the returned
    segment count matches `spec.md:5063-5095`'s template:
    closure-family non-captures = 4 segments; `VTableType` = 5
    segments; `VTableInstance` / `VTableDropShim` = 6 segments;
    `VTableMethodShim` = 8 segments; `ConstMemoryStatic` = 3
    segments.
15. **`synthetic_def_path_segments` -- excluded variants
    panic.** Calling on `SyntheticItemKind::None` or
    `SyntheticItemKind::ClosureCaptures { .. }` panics. Use
    `#[should_panic]` test attributes.
16. **`synthetic_def_path_hash` -- `StableCrateId(0)` marker.**
    For each non-excluded variant, the returned `DefPathHash`
    has `stable_crate_id == StableCrateId(0)`.
17. **`synthetic_def_path_hash` -- determinism.** Same input
    yields the same `DefPathHash` across two consecutive calls.
18. **`synthetic_def_path_hash` -- domain separation between
    families.** Two variants with otherwise-identical payload
    surrogates produce different `local_hash` values
    (e.g., `VTableType { principal: P }` vs
    `ClosureDropGlue { closure: H }` -- the variant-tag byte
    differentiates).
19. **`synthetic_def_path_hash` -- `ClosureCall` call_kind
    discriminant.** Three `ClosureCall` values sharing
    `closure` but differing on `call_kind` produce three
    distinct `local_hash` values.
20. **`synthetic_def_path_hash` -- excluded variants panic.**
    Same `#[should_panic]` cases as item 15 for
    `synthetic_def_path_hash`.

`cargo test -p regseq-model` passes.

## Review

After tests pass, request a Codex review using the four-block
template (per `SESSION.md` Review section):

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-73 against its task spec at
.harness/tasks/regseq-model/73-vtable-family-and-synthetic-keys.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/synthetic.rs
- crates/regseq-model/src/synthetic_def_path.rs
- crates/regseq-model/src/lib.rs

Focus on:
- Spec deviations (every rule in the Implementation Notes section).
- Correctness issues and generality loss.
- Design misalignment with the spec's public API / architecture.
- Test coverage vs. the spec's Test Plan.
Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs).
</task>

<grounding_rules>
Cite file:line for every claim and every required fix. Do not
speculate about code you did not read. Mark hypotheses as
hypotheses.
</grounding_rules>

<structured_output_contract>
First line: VERDICT: APPROVED | REQUIRES FIXES
If REQUIRES FIXES: numbered list, each entry exactly
  <file>:<line-range> -- <what is wrong> -- <what the spec requires>
If APPROVED: one paragraph (<=120 words) summarizing what was
verified and why any deliberate deviations are acceptable.
Total response <= 400 words.
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, form a verdict,
stop.
</default_follow_through_policy>
EOF
```

Iterate until VERDICT: APPROVED.

## Quality Gate

```
cargo test -p regseq-model
cargo fmt --all -- --check
cargo clippy -p regseq-model -- -D warnings
```

The authoritative gate is `task.py done` per `SESSION.md`.

## Acceptance Criteria

- `crates/regseq-model/src/synthetic.rs` extended with the
  vtable-family + const-memory `SyntheticItemKind` variants and
  the keying types `PrincipalTraitKey`, `ProjectionBindingsKey`,
  `ContentHash` per `spec.md:4821-4905`.
- New module `crates/regseq-model/src/synthetic_def_path.rs`
  created with the synthetic DefPath segment constructor and the
  synthetic `DefPathHash` constructor per `spec.md:5051-5184`.
  Re-exported from `crates/regseq-model/src/lib.rs`.
- The closed `SyntheticItemKind` enum has exactly 9 variants
  (`None` + 3 closure + 4 vtable + 1 const-memory) per
  `spec.md:4861-4905`.
- All public types carry doc comments including the collision-
  model framing for `ProjectionBindingsKey` and `ContentHash`
  per `spec.md:4942-4949` / `spec.md:4976-4981`.
- Test Plan items 1-20 implemented as `#[cfg(test)] mod tests`
  cases; all pass under `cargo test -p regseq-model`.
- `cargo fmt --all -- --check` and
  `cargo clippy -p regseq-model -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
