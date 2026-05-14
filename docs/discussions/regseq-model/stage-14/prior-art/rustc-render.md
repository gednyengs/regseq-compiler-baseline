# Rustc prior art -- top-level render

Evidence collected for **D-14-6** (top-level
`render(&self) -> String` composer renderer; iterates
`model.instances` and composes per-Instance renderings via
`render_instance` from D-14-5).

Toolchain: `nightly-2026-02-01-aarch64-apple-darwin`. Paths
are relative to:
`~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/`

The cluster surface under design (per `pretty-printer-snapshot.md`
§10.3 stub + `stage-14/memory.md` D-14-6 inventory):

- **D-14-6.1** -- API signature (`render(&self) -> String`).
- **D-14-6.2** -- iteration order over `model.instances`.
- **D-14-6.3** -- per-Instance separator.
- **D-14-6.4** -- file-level output envelope (header /
  metadata wrapper).
- **D-14-6.5** -- empty-model case.
- **D-14-6.6** -- cross-stage friction outcome.

D-14-5's `prior-art/rustc-instance.md` Q1 already surfaced
the upstream dispatch sites (`write_mir_pretty` +
`MonoItem::define` + `Instance::emit_mir`); this file
focuses on the **top-level batch-render** discipline rather
than per-Instance dispatch.

## Q1 -- Top-level entry: what is rustc's batch renderer?

**Finding.** `write_mir_pretty(tcx, single, w)` at
`rustc_middle/src/mir/pretty.rs:317-369` is the canonical
top-level entry. The full body:

```rust
pub fn write_mir_pretty<'tcx>(
    tcx: TyCtxt<'tcx>,
    single: Option<DefId>,
    w: &mut dyn io::Write,
) -> io::Result<()> {
    let writer = MirWriter::new(tcx);

    writeln!(w, "// WARNING: This output format is intended for human consumers only")?;
    writeln!(w, "// and is subject to change without notice. Knock yourself out.")?;
    writeln!(w, "// HINT: See also -Z dump-mir for MIR at specific points during compilation.")?;

    let mut first = true;
    for def_id in dump_mir_def_ids(tcx, single) {
        if first {
            first = false;
        } else {
            // Put empty lines between all items
            writeln!(w)?;
        }
        // ... trivial-const fast path OR write_mir_fn(body, w)
    }
    Ok(())
}
```

Shape: **3-line file-level WARNING comment header** + **for
loop over `dump_mir_def_ids(tcx, single)`** with **blank-line
separator between items** (no leading blank before the first
item; the `first` flag suppresses the separator on the first
iteration). Per-item dispatch is either a trivial-const
one-liner (`const PATH: TY = const VAL;`) or
`write_mir_fn(body, w)` -- the body envelope at
`pretty.rs:384-399` already covered by `rustc-instance.md`
Q2.

**Single-item mode.** The `single: Option<DefId>` parameter
lets a caller render exactly one body (e.g.,
`-Z dump-mir=PATH` filtering). When `single = Some(id)`,
`dump_mir_def_ids` returns `vec![id]`; when `None`, it
returns the full `tcx.mir_keys(())` set. The WARNING header
+ separator logic operates identically in both modes.

**Implication for D-14-6.** RegseqModel's top-level
`render(&self)` is the no-filter analog: it always renders
the full `model.instances` (no `single: Option<InstanceId>`
parameter in v1 -- single-Instance dispatch is already
covered by `render_instance(InstanceId)` from D-14-5). The
shape decisions inherit:

- **File-level header**: rustc emits 3 lines of WARNING +
  HINT comments before any item. D-14-6.4 must decide whether
  RegseqModel mirrors this, drops it, or emits a
  RegseqModel-specific metadata header.
- **Separator**: rustc emits a blank line between items, no
  leading blank. D-14-6.3 inherits this pattern as the
  evidence-preponderance default.
- **Empty-model case**: when `mir_keys` is empty, rustc
  emits only the 3-line WARNING header and no items. The
  output is non-empty but content-free. D-14-6.5 must decide
  whether RegseqModel mirrors this (header-only) or returns
  the empty string entirely.

## Q2 -- Iteration order: what order does rustc walk items?

**Finding.** Rustc's top-level MIR pretty-printer walks
`tcx.mir_keys(())` in **HIR-body-owner declaration order**,
NOT in any content-keyed sort. Defined at
`rustc_mir_transform/src/lib.rs:328-359`:

```rust
fn mir_keys(tcx: TyCtxt<'_>, (): ()) -> FxIndexSet<LocalDefId> {
    let mut set: FxIndexSet<_> = tcx.hir_body_owners().collect();
    set.retain(|&def_id| !matches!(tcx.def_kind(def_id), DefKind::GlobalAsm));
    // ... append coroutine-by-move bodies, tuple-struct ctors
    set
}
```

`FxIndexSet` preserves insertion order; insertion order is
`tcx.hir_body_owners()`, which walks the HIR in
source-declaration order. The set then has post-hoc
appends for coroutine-by-move bodies and tuple-struct
constructors (appended after the main hir_body_owners
sweep, so these synthetic items land at the end -- after
all source-declared bodies).

This is fundamentally a **source-order traversal**, not a
content-keyed sort. Two builds of the same source produce
the same order (the HIR walk is deterministic); two builds
of *different* sources (e.g., a refactor that reorders
functions) produce different orders.

**Codegen-layer ordering (separate from pretty-printer).**
`MonoItem::items_in_deterministic_order` at
`rustc_middle/src/mir/mono.rs:508-574` is the codegen-time
per-CGU sort, but it has **two modes** gated on
`tcx.sess.opts.unstable_opts.codegen_source_order`
(at `mono.rs:553`):

- **Default** (`codegen_source_order = false`): sort by
  `SymbolName` only (`mono.rs:571`,
  `items.sort_by_cached_key(|&(i, _)| i.symbol_name(tcx))`).
  The comment at `mono.rs:553-570` explains the rationale:
  the input is already deterministic; the symbol-name
  sort is a stable secondary key that helps LLVM cluster
  related monomorphizations.
- **`-Z codegen-source-order`** (off by default; on for
  codegen-test reproducibility): sort by the
  `(Option<Span>, SymbolName)` `ItemSortKey` defined at
  `mono.rs:514-550`. Spans take precedence when present;
  symbol name is the tiebreak.

Both modes are **supporting evidence only**, not
load-bearing for D-14-6.2. RegseqModel's `model.instances`
is sorted by F4-2 (`RegseqInstanceKey`) at construction,
which is strictly more deterministic than either rustc
mode (content-keyed across the full build-equivalence
quad). The codegen-tier ordering is cited here to
illustrate that rustc's own consensus is "sort at the
upstream layer; don't re-sort at every consumer."

**Stable-IR analog.** `rustc_public` (`rustc_public/src/mir/pretty.rs`)
has **NO** top-level batch renderer. The closest is
`Instance::emit_mir(&self, w)` at
`rustc_public/src/mir/mono.rs:178-180` -- per-Instance only.
Stable-IR consumers must write their own iteration loop
over `crate.collect_mono_items()` or
`Instance::mono_items()` and invoke `emit_mir` per Instance.

**Implication for D-14-6.2.** RegseqModel's
`model.instances` is **already F4-2 sorted** at the
IR-construction layer per `spec.md` §16.3
(`RegseqInstanceKey = (regseq_kind_rank, item_id,
generic_args_key, fn_shim_disambig)`). The IndexVec stores
Instances in canonical order; the rendering layer does NOT
need to re-sort.

The choice for D-14-6.2 is therefore:

- **(A) Walk `instances.iter_enumerated()` verbatim** --
  iterate in InstanceId order, which IS F4-2 sorted by
  construction. Stable cross-build for identical sources +
  toolchain + target. No per-render sort.
- **(B) Re-sort by some other key at render time** (e.g.,
  `regseq_instance_kind_rank` grouping, lexical DefPath,
  span-based source order). Adds a render-time pass; only
  necessary if the F4-2 order produces poor diagnostic
  readability.

Rustc's `write_mir_pretty` walks `mir_keys` (HIR-walk
source order) directly, no re-sort. The F4-2 order is
strictly more deterministic than `mir_keys` (content-keyed
vs source-order), so (A) inherits rustc's
"walk-the-canonical-order" pattern with strictly stronger
determinism guarantees. (B) would be motivated only by
readability -- but F4-2 already groups by InstanceKind
rank first (Functions before Statics before Consts before
Structs/Enums/Unions per §9.3's
`regseq_instance_kind_rank`), then by ItemId within rank.
This is already the rank-grouped order that diagnostic
diffs benefit from.

**Recommendation:** (A) walk `instances.iter_enumerated()`
in InstanceId order. The F4-2 sort key in §16.3 already
provides both the cross-build stability AND the
rank-grouped readability that (B) would manually
reconstruct.

## Q3 -- Per-item separator: what goes between consecutive items?

**Finding.** Rustc's `write_mir_pretty` emits exactly one
blank line between consecutive items, with no leading
blank before the first item. The mechanism is the
boolean `first` flag at `pretty.rs:328-335`:

```rust
let mut first = true;
for def_id in dump_mir_def_ids(tcx, single) {
    if first {
        first = false;
    } else {
        // Put empty lines between all items
        writeln!(w)?;
    }
    // ... render this item
}
```

The comment "Put empty lines between all items" + the
single `writeln!(w)?` produces exactly one `\n` between
items -- but each item's own renderer (e.g.,
`write_mir_fn`) ends with its closing `}` followed by an
implicit newline from the writer, so the visual effect is
**one blank line between items** (the closing `\n` from
the prior item + the `\n` from the separator).

**Special-cased "// MIR FOR CTFE" interstitial.** When
rendering a `const fn`, rustc emits both `optimized_mir`
and `mir_for_ctfe` for the same DefId, separated by a
single blank line + the comment `// MIR FOR CTFE`
(`pretty.rs:348-354`). RegseqModel has no analog -- each
RegseqModel `Instance` is a single emission unit; the
ctfe-vs-runtime split lives at the rustc layer below
RegseqModel.

**No item-header comment.** Rustc does NOT emit a per-item
header comment like `// Item: foo::bar` before each body.
The first rendered line of each per-Instance render IS the
header -- in rustc that is the keyword line because the MIR
pretty-printer does not emit a per-Item attribute block; in
RegseqModel that is the Stream-1 + Stream-2 attribute block
when non-empty (per §10.2.7) and otherwise the kind-keyword
line. Either way, the per-Instance renderer is responsible
for self-identification.

**Implication for D-14-6.3.** Three candidate forms:

- **(A) Top-level composer emits `"\n\n"` between
  consecutive `render_instance(id)` outputs.** RegseqModel's
  per-Instance renderers end at `}` or `;` with no trailing
  newline (per the §9.11 specimens); the composer owns the
  full two-byte separator. First `\n` terminates the prior
  Instance's last line; second `\n` produces the single
  blank line. No leading or trailing separator (first-flag
  pattern adapted from rustc).
- **(B) Header-comment + blank line** -- e.g., `// =====
  Function: foo::bar =====\n` before each body. Adds
  scan-friendly separators but introduces noise and
  duplicates each Instance's self-identifying prefix
  (attribute block when non-empty; otherwise the kind-
  keyword line).
- **(C) No separator (concatenation)** -- places the next
  Instance's first byte (attribute block or keyword) on the
  same line as the prior Instance's closing `}` or `;`.
  Bad readability.

Rustc's first-flag separator pattern at `pretty.rs:328-335`
is the upstream precedent; the underlying byte-sequence
diverges because RegseqModel's per-Instance renderers don't
carry trailing newlines (whereas rustc's writer flush
appends implicit `\n` after each body's closing `}`). The
composer-owns-the-separator design respects RegseqModel's
no-trailing-newline contract directly.

**Recommendation:** (A) -- composer-owned `"\n\n"`
separator; no leading or trailing separator.

## Q4 -- File-level output envelope: header / wrapper?

**Finding.** Rustc emits a **3-line WARNING + HINT comment
header** at the top of the output (`pretty.rs:324-326`):

```
// WARNING: This output format is intended for human consumers only
// and is subject to change without notice. Knock yourself out.
// HINT: See also -Z dump-mir for MIR at specific points during compilation.
```

The header is unconditional (no flag gating). It exists to
deter consumers from parsing `--emit=mir` output
mechanically. The HINT line points to the alternative
intermediate-stage dump facility for users wanting MIR at
specific compilation phases.

No file-level **trailer** -- the output ends with the last
item's closing `}` + implicit newline.

**Stable-IR analog.** `rustc_public` has no top-level
batch renderer (Q1 finding), so no analog file-level
header. Consumers writing their own iteration loop must
write their own header if desired.

**Implication for D-14-6.4.** Four candidate forms:

- **(A) No file-level header.** Output starts with the
  first rendered byte of the first `render_instance` output
  (Stream-1 + Stream-2 attribute block when non-empty per
  §10.2.7, otherwise the kind-keyword line); ends at the
  last rendered byte (`}` or `;`) with no trailing newline
  injected. Cleanest; matches rustc_public's per-Instance
  composition pattern.
- **(B) Rustc-mirrored 3-line WARNING + HINT header.**
  Pasted verbatim with `RegseqModel` substituted for `MIR`
  where appropriate. Explicit "do not parse mechanically"
  signal.
- **(C) RegseqModel-specific metadata header.** E.g.,
  crate name + toolchain + target + emission timestamp.
  Useful for diagnostic traceback but adds noise; the
  same metadata is already in `RegseqModel.build_info`
  (§3.1) and accessible to consumers without re-rendering.
- **(D) Minimal version-tag header.** One-line `//
  RegseqModel pretty-print v1` for forward-compatibility
  signalling. Less noise than (B) or (C).

The rustc-derived motivation for (B) is "format is
unstable, do not parse." RegseqModel's
`pretty-printer.md` is **explicitly designed as a snapshot
artifact for `insta` snapshot tests + diagnostic output**
per Q5 / stage-1 D-1-S2. The output IS the spec; format
stability is enforced by the snapshot test suite. The
rustc warning's premise (consumers might think MIR pretty-
print is parseable) does not apply.

(C) overlaps with `RegseqModel.build_info` which already
holds the same data; emitting it in the pretty-printed
output would duplicate it without benefit. A diagnostic
consumer wanting the build provenance reads
`model.build_info` directly.

(D) is the minimal version-signal that supports
forward-compatibility -- if a future stage revises the
output format, consumers reading the snapshot can detect
the version transition. But the snapshot suite (insta)
already enforces format stability under the pinned
toolchain; the version signal is operationally redundant.

**Recommendation:** (A) no file-level header / trailer.
The output starts at the first rendered byte of the first
`render_instance` output (attribute block when non-empty,
otherwise the keyword line); ends at the last rendered byte
(`}` or `;`) with no trailing newline. This matches the
rustc_public per-Instance composition shape (where
stable-IR consumers explicitly own the file-level wrapper
layer) and aligns with the consumer model: pretty-printed
output is consumed by snapshot tests + diagnostic
facilities, both of which manage their own framing.

## Q5 -- Empty-model case

**Finding.** Rustc's `write_mir_pretty` handles the empty
`mir_keys` case by emitting the 3-line WARNING header and
**nothing else**. The output is non-empty (header-only)
but content-free. The `first` flag never flips, so no
separator is ever emitted.

This is operationally meaningful in rustc only for
crates with zero HIR body-owners after the
`GlobalAsm` filter at
`rustc_mir_transform/src/lib.rs:334`. Const items DO
appear in `mir_keys` (they are HIR body-owners, with
the trivial-const fast path at `pretty.rs:356-361`
producing the one-liner `const PATH: TY = const VAL;`)
and so do NOT trigger the empty case. Type-only crates
(only `struct` / `enum` / `union` / `type` aliases) +
crates with only foreign declarations are the
practical empty cases.

**Implication for D-14-6.5.** Per Q4 above, RegseqModel's
recommended (A) (no file-level header) means the natural
empty-model output is **the empty string** -- there is no
header to emit and no items to iterate. The output is
`""`.

Two candidate forms:

- **(A) Empty string `""`.** Falls out of (Q4-A) + an
  empty `model.instances`. Natural identity for the
  concatenation operation. Snapshot tests of empty models
  produce a 0-byte snapshot.
- **(B) Placeholder comment** like `// (empty model)`.
  Surfaces intent for "this is an intentionally empty
  build artifact, not a truncated render." But adds noise
  for every valid empty case (test fixtures with bare
  `RegseqModel::new()`, foundation-crate-only builds
  before phase-2 features land, etc.).

Empty `model.instances` arises legitimately in:
- Foundation-crate test fixtures that exercise pure-Item
  surfaces (DefPath construction, IntrinsicKind
  vocabulary, Layout schema) without monomorphizing any
  bodies.
- Future stages' validators / serialization round-trips
  on empty-construction baselines.
- Test fixtures that exercise rejection paths (the model
  is never populated because construction errors out).

In all three, the consumer expects "no Instances => no
output." A placeholder comment adds confusion ("did
something get filtered? is this a stub?") without
operational value.

**Recommendation:** (A) empty string `""`.

## Q6 -- API signature: arguments + return type

**Finding.** Rustc's `write_mir_pretty(tcx, single,
w)` takes three arguments: the `TyCtxt` (the rustc
typing context), a `single: Option<DefId>` filter, and
a `w: &mut dyn io::Write` sink. Output goes to `w`
via `writeln!` / `write!`; the return type is
`io::Result<()>`.

`rustc_public` chose a **per-Instance write-sink form**
at `Instance::emit_mir(self, w)` (`mono.rs:178-180`).
Same `&mut W: io::Write` sink shape, but per-Instance,
not top-level. No top-level batch renderer exists.

**Implication for D-14-6.1.** Per `pretty-printer-snapshot.md`
§2.3 (D-13-1), RegseqModel's renderers are **methods on
`PrettyPrinter<'a>` returning `String`** -- not write-sink
form. The decision is downstream of D-13-1; D-14-6.1
inherits.

Per `pretty-printer-snapshot.md` §1.3 line 116-118 and §8
ownership table line 1143, `render(&self) -> String` is
already pinned by stage-13 D-13-4.6 + D-14-5's §1.3
finalisation. D-14-6.1's role is to **confirm + lock**
this signature against the cluster's per-arm composition
contract, not to revisit the architectural choice.

The single-argument form (no `single:
Option<InstanceId>` filter) is the v1 commitment:
single-Instance dispatch is already covered by
`render_instance(InstanceId)` from D-14-5; a top-level
filter would duplicate that surface. Future revision per
"Spec is mutable" if a hot-path consumer surfaces.

**Recommendation:** Confirm + lock
`render(&self) -> String`. No `single` parameter; no
write-sink form; no `Option` return wrapping. Composes
per-Instance content via `render_instance(InstanceId)`
applied across `self.model.instances.iter_enumerated()`.

## Summary -- per-sub-decision evidence preponderance

| Sub-decision | Question                          | Evidence-preponderance recommendation                                              |
|--------------|-----------------------------------|------------------------------------------------------------------------------------|
| D-14-6.1     | API signature                     | `render(&self) -> String` (confirm + lock; no `single` filter; no write-sink form) |
| D-14-6.2     | Iteration order                   | Walk `instances.iter_enumerated()` in InstanceId order (F4-2 sorted by construction) |
| D-14-6.3     | Per-item separator                | Single blank line between items; no leading blank; no trailing blank               |
| D-14-6.4     | File-level output envelope        | No header / no metadata wrapper (consumers manage framing)                         |
| D-14-6.5     | Empty-model case                  | Empty string `""`                                                                  |
| D-14-6.6     | Cross-stage friction outcome      | No anticipated stage-13 REOPENs (top-level composition is consumer-side only)      |

The cluster surface is **strictly smaller** than D-14-4 +
D-14-5 because the four content decisions (Q2-Q5) all
have rustc evidence preponderance or simple identity-based
answers; only Q2 (iteration order) is genuinely
load-bearing, and the F4-2 sort already in `spec.md` §16.3
provides the answer without re-deriving.

## When to consult

- Designing the `render(&self)` per-arm composition outline ->
  Q1 (top-level entry shape).
- Choosing iteration order over `model.instances` -> Q2 (F4-2
  already provides; no re-sort needed).
- Choosing per-Instance separator form -> Q3 (single blank
  line, no header).
- Choosing file-level envelope / wrapper -> Q4 (no header
  recommended).
- Handling empty `model.instances` -> Q5 (empty string).
- Confirming API signature -> Q6 (single-arg form locked
  upstream; D-14-6.1 confirms).

## What's NOT in this inventory

- Per-Instance rendering details (`render_instance`'s
  per-variant per-arm specs). Already covered by D-14-5 +
  `prior-art/rustc-instance.md`.
- Body-level rendering details (statement, terminator,
  rvalue, operand, place per-arm specs). Already covered by
  stage-13 D-13-6.
- Const-rendering details. Already covered by D-14-4 +
  `prior-art/rustc-const.md`.
- Codegen-time emission order (per-CGU
  `items_in_deterministic_order`). Cited as supporting
  evidence in Q2; not load-bearing because RegseqModel
  rendering is pretty-printer-tier (snapshot artifact), not
  codegen-tier (LLVM IR emission).
