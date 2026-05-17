# regseq-model-96: Deterministic `DomainAttrTables` JSON Serialization (`HashMap` → `BTreeMap`)

## Description

Close the JSON output non-determinism documented as a Non-Goal
in `regseq-driver-52`'s task spec: when
`domain_attrs.field_attrs` and `domain_attrs.span_attrs` are
non-empty, the serialized JSON byte-form is non-deterministic
across runs because the `hash_map_vec::serialize` adapter at
`crates/regseq-model/src/domain_attr.rs:104-130` collects
`HashMap::iter()` without sorting, and `HashMap` iteration order
is intentionally randomized.

All four phase-1a fixtures (`no_op`, `diag_logging`,
`trait_object`, `closure`) produce empty `field_attrs` /
`span_attrs` maps today, so the determinism gap is dormant. It
surfaces the moment any fixture (or any phase-1b code) populates
either map -- making the JSON output unsuitable for
diff-based regression testing, content-addressable storage, or
build-cache keying.

This task replaces the two `HashMap`-backed fields with
`BTreeMap`, threading the required `Ord` derives through `Span`
and `FieldPath` (Codex pre-task review verified the
newtype-index types already hand-write `Ord` over `as_u32()` at
`crates/regseq-model/src/idx.rs:177-192`, so the
`newtype_index!`-generated types `ItemId`, `SourceFileId`,
`VariantIdx`, `FieldIdx` are already Ord-comparable).

The array-of-pairs JSON shape is PRESERVED. `FieldPath` and
`Span` are struct keys (not strings); JSON object keys must be
strings; therefore the only well-formed JSON representation is
`Vec<(K, V)>`, which is exactly what `hash_map_vec` emits today.
The fix renames the adapter to reflect its new ordered-map
input but keeps the output shape identical -- preserving
round-trip compatibility with any existing serialized
artifacts.

After this task lands, the `regseq-driver-52` JSON
reproducibility Non-Goal is dropped: `cargo regseq --emit
json` produces byte-identical output for byte-identical input
regardless of how many `field_attrs` / `span_attrs` entries
the bridge populates.

## Input Context

- `crates/regseq-model/src/domain_attr.rs:14-35`
  -- `DomainAttrTables` struct: the two `HashMap`-backed
  fields targeted by this task (`field_attrs: HashMap<FieldPath,
  Vec<AttrData>>` at line 27, `span_attrs: HashMap<Span,
  Vec<AttrData>>` at line 34, both with
  `#[serde(with = "hash_map_vec")]`).
- `crates/regseq-model/src/domain_attr.rs:37-46`
  -- `FieldPath` struct (derive list at line 38 needs
  `Ord, PartialOrd` added).
- `crates/regseq-model/src/domain_attr.rs:104-130`
  -- the `hash_map_vec` serde adapter module that this task
  renames + retypes for `BTreeMap` input.
- `crates/regseq-model/src/source.rs:110-124`
  -- `Span` struct (derive list at line 117 needs
  `Ord, PartialOrd` added). Field order is `(file, lo, hi)`,
  which yields the natural lex order on derive.
- `crates/regseq-model/src/idx.rs:121,177-192`
  -- `newtype_index!` macro: derive list at line 121
  intentionally excludes `Ord` because `NonMaxU32`'s encoded
  order is reversed; hand-written `PartialOrd` + `Ord` impls
  at lines 177-192 give the correct logical-index order via
  `as_u32()`. No change needed to the macro; the surrounding
  types already work.
- `crates/regseq-model/src/idx.rs:15-27`
  -- the `Idx` trait already requires `Ord` and documents
  logical-index ordering as the contract.
- `crates/regseq-model/src/model.rs:1158-1170`
  -- `for_each_item_id_mut`: line 1162 calls
  `self.domain_attrs.field_attrs.drain().collect()`.
  `BTreeMap` doesn't have a stable `.drain()` method; this
  call site must change to
  `std::mem::take(&mut self.domain_attrs.field_attrs).into_iter().collect()`.
- `crates/regseq-model/src/domain_attr.rs:271-330`
  -- existing test patterns (`field_path_hash_map_key_behavior`
  + adjacent round-trip tests) -- the natural neighborhood
  for the new determinism test.
- `crates/regseq-model/src/string_interner.rs:119-126`,
  `crates/regseq-model/src/fn_sig_interner.rs:145-151`,
  `crates/regseq-model/src/ty_interner.rs:294-300`,
  `crates/regseq-model/src/const_interner.rs:127-133`
  -- reference: every interner already hides its lookup
  `HashMap` behind a custom `Serialize` that emits only the
  ordered `storage` IndexVec; this is the pattern this task
  brings to `DomainAttrTables`.
- `crates/regseq-model/src/model.rs:91-104`
  -- `RegseqModel.vtables` and `closure_captures_struct_ty`
  are `#[serde(skip)]`, rebuilt on deserialize; they do NOT
  affect JSON output reproducibility and are out of scope.
- `docs/discussions/regseq-model/spec.md` §3 (RegseqModel
  root + sibling tables) + §3.6
  (`DomainAttrTables`) -- the spec defines the structural
  shape; it does NOT mandate a non-natural ordering for
  serialization, so lex order by struct fields is
  spec-compatible.
- `.harness/tasks/regseq-driver/52-emit-dispatcher-wiring.md`
  -- the upstream task this fix's Non-Goal carve-out
  references; after this task lands, that Non-Goal is
  dropped.

## Implementation Notes

Four changes in dependency order so each is testable in
isolation.

### Change A: Add `Ord, PartialOrd` to `Span`

In `crates/regseq-model/src/source.rs:117`, extend the derive
list:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Span {
    pub file: SourceFileId,
    pub lo: u32,
    pub hi: u32,
}
```

The derived `Ord` performs field-order lex comparison:
`(file, lo, hi)`. `SourceFileId`'s `Ord` impl is the
hand-written one at `idx.rs:177-192` (logical-index order via
`as_u32()`), so spans are first ordered by file, then by `lo`,
then by `hi`. This is the natural canonical ordering for
spans within a model.

### Change B: Add `Ord, PartialOrd` to `FieldPath`

In `crates/regseq-model/src/domain_attr.rs:38`, extend the
derive list:

```rust
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct FieldPath {
    pub item: ItemId,
    pub variant: VariantIdx,
    pub field: FieldIdx,
}
```

Derived lex order over `(item, variant, field)`. All three
component types already have the hand-written `Ord` impl from
the `newtype_index!` macro. This ordering matches the
canonical iteration order one would walk: items first
(F4-sorted post-Phase-4), then variants within an item, then
fields within a variant.

### Change C: Replace `HashMap` with `BTreeMap`; rename and retype the serde adapter

In `crates/regseq-model/src/domain_attr.rs`:

1. **Line 3**: Change the import:
   ```rust
   use std::collections::BTreeMap;
   ```
   (`HashMap` may still be needed elsewhere in the module; if
   so, keep both imports.)
2. **Lines 27, 34**: Replace the field types:
   ```rust
   #[serde(with = "ordered_map_vec")]
   pub field_attrs: BTreeMap<FieldPath, Vec<AttrData>>,

   #[serde(with = "ordered_map_vec")]
   pub span_attrs: BTreeMap<Span, Vec<AttrData>>,
   ```
3. **Lines 104-130**: Rename the `hash_map_vec` module to
   `ordered_map_vec` and update its function signatures to
   accept `BTreeMap<K, V>`:

   ```rust
   mod ordered_map_vec {
       use super::*;
       use serde::{Deserializer, Serializer};
       use std::collections::BTreeMap;

       /// Serialize a `BTreeMap` as an array of (key, value)
       /// pairs. The array-of-pairs JSON shape is required
       /// because `FieldPath` and `Span` are struct keys
       /// (not strings), and JSON object keys must be strings.
       /// BTreeMap iteration is naturally sorted by key, so
       /// the emitted Vec is deterministic across runs.
       pub fn serialize<K, V, S>(
           map: &BTreeMap<K, V>,
           serializer: S,
       ) -> Result<S::Ok, S::Error>
       where
           K: Serialize + Ord,
           V: Serialize,
           S: Serializer,
       {
           let entries: Vec<(&K, &V)> = map.iter().collect();
           entries.serialize(serializer)
       }

       /// Deserialize an array of (key, value) pairs into a
       /// `BTreeMap`.
       pub fn deserialize<'de, K, V, D>(
           deserializer: D,
       ) -> Result<BTreeMap<K, V>, D::Error>
       where
           K: Deserialize<'de> + Ord,
           V: Deserialize<'de>,
           D: Deserializer<'de>,
       {
           let entries: Vec<(K, V)> = Vec::deserialize(deserializer)?;
           Ok(entries.into_iter().collect())
       }
   }
   ```

   Key differences from the old `hash_map_vec`:
   - Function signature: `BTreeMap<K, V>` (was `HashMap<K, V>`).
   - Trait bound on `K`: `Ord` (was `Eq + Hash`).
   - Iteration order: naturally sorted (was insertion-order /
     random per HashMap's default state).
   - Output JSON shape: UNCHANGED (still `Vec<(K, V)>`).

### Change D: Replace `field_attrs.drain()` with `std::mem::take` + update stale doc comment

In `crates/regseq-model/src/model.rs:1162`:

Replace:
```rust
let drained: Vec<_> = self.domain_attrs.field_attrs.drain().collect();
```

With:
```rust
let drained: Vec<_> = std::mem::take(&mut self.domain_attrs.field_attrs)
    .into_iter()
    .collect();
```

`std::mem::take` swaps the field out for `BTreeMap::default()`
(empty) and returns the original; `.into_iter()` consumes the
returned map. Behaviorally equivalent to the old `.drain()`
call (both empty the map and yield owned `(K, V)` pairs); the
only difference is `std::mem::take` works on both `HashMap`
and `BTreeMap` while `.drain()` is `HashMap`-specific in
stable Rust.

The reinsert at line 1165 (`self.domain_attrs.field_attrs.insert(key, value)`)
works on both map types unchanged.

**Doc-comment fix (REQUIRED).** The doc-comment block at
`crates/regseq-model/src/model.rs:1132-1135` describes the
side-table remap path and currently says:

```text
/// - Side-table keys: `domain_attrs.field_attrs`'s
///   `FieldPath.item` field. The HashMap is rebuilt with
///   remapped keys so subsequent lookups find the entries
///   under their new `FieldPath`.
```

After this change the underlying type is `BTreeMap`. Update
"The HashMap is rebuilt" to "The map is rebuilt" (or
explicitly "The BTreeMap is rebuilt"); the rebuild semantics
are unchanged.

### Conventions

- The `ordered_map_vec` adapter MUST keep the array-of-pairs
  JSON shape (not BTreeMap's default `Serialize` impl). The
  shape is forced by `FieldPath`/`Span` being struct keys
  (JSON requires string keys for object form).
- Doc comments on `Ord`-derived structs cite the natural
  lex order they yield (per Changes A + B).
- The renamed adapter's doc-comment cites BTreeMap's
  natural-sort guarantee.

## Non-Goals

- **Bridge-side population of `field_attrs` / `span_attrs`.**
  The current phase-1a fixtures all produce empty maps; this
  task does NOT add fixtures that populate them. A future
  fixture (e.g. introducing `#[rseq::<field-level-attr>]`
  recognition) is its own task.
- **Determinism for `HashMap` fields outside `DomainAttrTables`.**
  Codex pre-task review confirmed that every other HashMap in
  `RegseqModel` is either `#[serde(skip)]` (rebuilt on
  deserialize) or has a custom `Serialize` impl that emits only
  ordered backing storage (interners). No other determinism
  fix is in scope.
- **Cross-version JSON deserialization compatibility.** This
  task does NOT add a JSON schema-version field or migration
  layer. The serialized JSON shape (array of pairs) is
  unchanged, so existing serialized artifacts remain readable.
- **Performance optimization of `field_attrs.get(...)`.** The
  HashMap → BTreeMap switch trades O(1) avg for O(log n)
  lookup. At expected sizes (tens to low hundreds of entries),
  the cost is negligible. Benchmarking is out of scope.
- **The `regseq-driver-52` Non-Goal text amendment** (which
  flags this gap) is OUT of scope here; that's a regseq-driver-52
  spec edit that lands separately if and when this task closes.
- **Deriving `Ord` on `AttrData` / `IntrinsicKind` / `LogLevel`.**
  The map values are `Vec<AttrData>`; map order is determined
  by KEY ordering only. Value-side `Ord` is not needed for
  deterministic serialization.

## Test Plan

### A. Compilation gate

1. `cargo build --workspace` succeeds after Changes A-D.
   (Catches any callers of `field_attrs.drain()` outside
   `model.rs:1162` that the implementation missed.)

### B. Deterministic JSON across insertion orders

2. New unit test in `crates/regseq-model/src/domain_attr.rs`
   (placement near `field_path_hash_map_key_behavior` at line
   271, or near the existing round-trip tests at lines
   355-375 per Codex's recommendation):

   ```rust
   #[test]
   fn domain_attrs_json_is_deterministic_across_insert_order() {
       let entries = [
           (
               FieldPath { item: ItemId::from_u32(5), variant: VariantIdx::ZERO, field: FieldIdx::from_u32(2) },
               vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Debug))],
           ),
           (
               FieldPath { item: ItemId::from_u32(2), variant: VariantIdx::ZERO, field: FieldIdx::from_u32(7) },
               vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Info))],
           ),
           (
               FieldPath { item: ItemId::from_u32(9), variant: VariantIdx::ZERO, field: FieldIdx::from_u32(0) },
               vec![AttrData::Intrinsic(IntrinsicKind::Log(LogLevel::Warn))],
           ),
       ];

       // Forward insertion order.
       let mut a = DomainAttrTables::default();
       for (k, v) in entries.iter() {
           a.field_attrs.insert(k.clone(), v.clone());
       }

       // Reverse insertion order.
       let mut b = DomainAttrTables::default();
       for (k, v) in entries.iter().rev() {
           b.field_attrs.insert(k.clone(), v.clone());
       }

       let json_a = serde_json::to_string(&a).unwrap();
       let json_b = serde_json::to_string(&b).unwrap();
       assert_eq!(json_a, json_b, "JSON output must be byte-identical regardless of insertion order");
   }
   ```

3. Add the analogous test for `span_attrs` using `Span`
   values with distinct `(file, lo, hi)` triples.

### C. Round-trip deserialization preserves entries

4. Build a `DomainAttrTables` with 3+ `field_attrs` entries
   and 3+ `span_attrs` entries; serialize to JSON;
   deserialize back; assert the deserialized map contains
   the same `(key, value)` set (set equality, not
   sequence equality -- but with BTreeMap iteration order
   should be deterministic post-deserialize too).

### D. `for_each_item_id_mut` ItemId remap still works

5. The existing test
   `crates/regseq-model/src/model_tests.rs:1597-1619`
   (`domain_attrs_field_attrs_key_is_remapped`) MUST still
   pass without modification. Per Codex's pre-implementation
   review, this test:
   - Inserts a `field_attrs` entry whose `FieldPath.item`
     points at some `ItemId`.
   - Calls `model.sort_items_by_f4(...)`, which invokes
     `for_each_item_id_mut` (`crates/regseq-model/src/model.rs:1195`
     and `:1224-1227`).
   - Asserts the entry now lives under the remapped
     `FieldPath` (i.e., the `(ItemId, VariantIdx, FieldIdx)`
     key was rebuilt with the new `ItemId`).

   The `std::mem::take(&mut field_attrs).into_iter().collect()`
   substitution in Change D must preserve this behavior. The
   test asserting the remapped key is found via `BTreeMap::
   get(...)` works identically to `HashMap::get(...)`, so no
   test changes are required.

   If `cargo test -p regseq-model
   domain_attrs_field_attrs_key_is_remapped` fails after the
   substitution, the take-and-reinsert pattern was wired
   incorrectly; fix Change D until it passes.

### E. Workspace regression

6. `cargo test --workspace` -- all existing tests still pass.
   Particularly verify:
   - `regseq-model-94`'s diag_logging E2E test
     (`crates/regseq-model/tests/e2e_diag_logging.rs`) still
     passes -- it asserts `IntrinsicKind::Log(_)` lives in
     `domain_attrs.item_attrs` (IndexVec, unaffected) but
     touches `domain_attrs` accessors broadly.
   - `regseq-model-95`'s synthesis E2E test
     (`crates/regseq-model/tests/e2e_synthesis.rs`) still
     passes.
   - `regseq-driver-49`'s validator tests pass.
7. `cargo fmt --all -- --check` -- clean.
8. `cargo clippy --workspace --tests -- -D warnings` -- no
   new lint sites beyond the pre-existing baseline.

## Review

Once tests pass, request a Codex review of the implementation
by invoking `scripts/python/codex_runner.py` via the Bash tool
(per `SESSION.md` Review section). Use the four-block template
verbatim:

```bash
cat <<'EOF' | uv run python scripts/python/codex_runner.py --timeout 540
<task>
Review the implementation of regseq-model-96 against its task
spec at
.harness/tasks/regseq-model/96-deterministic-domain-attrs-json-serialization.md.
ALL CHECKS MUST BE SCOPED TO THE TASK DEFINED BY THE TASK SPEC.

Files to audit:
- crates/regseq-model/src/source.rs (Span Ord derive)
- crates/regseq-model/src/domain_attr.rs (FieldPath Ord
  derive; HashMap -> BTreeMap; ordered_map_vec module)
- crates/regseq-model/src/model.rs (drain -> mem::take at
  line ~1162)

Focus on:
- The Ord derives on Span and FieldPath MUST yield the
  natural lex order over the documented field sequence
  ((file, lo, hi) for Span; (item, variant, field) for
  FieldPath). No hand-written Ord impl that diverges from
  the derive.
- The ordered_map_vec adapter MUST preserve the array-of-
  pairs JSON shape (Vec<(K, V)>). Not BTreeMap's default
  Serialize.
- The drain replacement MUST use std::mem::take, not any
  alternative that might change semantics or panic on
  empty maps.
- All Test Plan items A1-E8 are present.
- The unit test at B2-B3 asserts byte-identical JSON
  across distinct insertion orders.

Do NOT flag anything explicitly marked out of scope (Non-Goals,
later task IDs).
</task>

<grounding_rules>
Cite file:line for every claim. No speculation. Mark
hypotheses as hypotheses.
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

Iterate (fix → re-invoke `codex_runner.py`) until VERDICT:
APPROVED.

## Quality Gate

Single-crate (`regseq-model`); the bridge depends transitively
but does not call the affected types in HashMap-specific ways
(verified by Codex pre-task review at Q5):

```
cargo test --workspace
cargo fmt --all -- --check
cargo clippy --workspace --tests -- -D warnings
```

The authoritative gate is `task.py done`, which runs cleanup
plus workspace-wide test/fmt/clippy per `SESSION.md` Testing
Requirements.

## Acceptance Criteria

- `crates/regseq-model/src/source.rs:117` `Span` derives
  include `PartialOrd, Ord` (field-order lex via
  `(file, lo, hi)`).
- `crates/regseq-model/src/domain_attr.rs:38` `FieldPath`
  derives include `PartialOrd, Ord` (field-order lex via
  `(item, variant, field)`).
- `crates/regseq-model/src/domain_attr.rs:27,34` types are
  `BTreeMap<FieldPath, Vec<AttrData>>` and
  `BTreeMap<Span, Vec<AttrData>>`; both fields carry
  `#[serde(with = "ordered_map_vec")]`.
- `crates/regseq-model/src/domain_attr.rs:104+` carries the
  renamed `ordered_map_vec` module whose `serialize`
  signature accepts `&BTreeMap<K, V>` with `K: Ord` and
  emits `Vec<(&K, &V)>` (array-of-pairs JSON shape
  unchanged).
- `crates/regseq-model/src/model.rs:1162` uses
  `std::mem::take(...)` instead of `.drain()`.
- `crates/regseq-model/src/model.rs:1132-1135` doc-comment
  updated from "The HashMap is rebuilt" to "The map is
  rebuilt" (or equivalent BTreeMap-correct wording).
- The existing test
  `domain_attrs_field_attrs_key_is_remapped`
  (`crates/regseq-model/src/model_tests.rs:1597-1619`)
  passes unmodified after Change D.
- The new unit test (Test Plan B2-B3) asserts byte-identical
  JSON across distinct insertion orders for both `field_attrs`
  and `span_attrs`.
- `cargo test --workspace` regression-clean.
- `cargo fmt --all -- --check` and
  `cargo clippy --workspace --tests -- -D warnings` pass.
- Codex Task Review returns VERDICT: APPROVED.
