# rustc `#[naked]` handling -- evidence pass for D-14-7

Stage-14 D-14-7 cluster framing per `../README.md` §2 "Per-stage
prior-art research uses primary sources." All paths are relative
to the pinned toolchain:

```
~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/
```

This file captures what rustc surfaces for `#[naked]` so that
D-14-7's "reject `#[naked]` Items at Item translation" decision
can be grounded in concrete rustc-side mechanics: the recognition
API the bridge calls, the codegen consequence rustc derives from
the flag, the body-shape contract a `#[naked]` function carries,
and the structural overlap with the InlineAsm-terminator
rejection already locked at stage-9 D-9-9 row 8.

## Q-tag inventory

- **Q1** -- Attribute parsing site. Where in rustc's HIR-attr
  pipeline does `#[naked]` get parsed into the typed shape the
  bridge later observes?
- **Q2** -- Source-of-truth flag. Which rustc API does the
  bridge call to ask "does this DefId carry `#[naked]`?" Is it
  the parsed `AttributeKind::Naked` HIR shape, or the
  downstream `CodegenFnAttrFlags::NAKED` bit?
- **Q3** -- MIR body shape under `#[naked]`. What does the
  body of a `#[naked]` function actually look like in
  `optimized_mir`? Does it carry user-readable MIR or asm-only
  content?
- **Q4** -- Codegen-side dispatch. How does rustc itself
  dispatch `#[naked]`-flagged MonoItems through codegen?
  What does the codegen path emit instead of an ordinary
  function body?
- **Q5** -- Attribute-target restrictions. Where can rustc
  source legally place `#[naked]`? What are the legality
  rules the bridge can rely on having already been enforced
  before its observation point?
- **Q6** -- Structural overlap with stage-9 D-9-9 row 8
  (`TerminatorKind::InlineAsm` rejection). If the bridge did
  NOT reject `#[naked]` at Item translation, would the body
  walk reject it later? What's the residual incentive for
  Item-level rejection over deferring to the existing
  terminator-level rejection?

## Q1 -- Attribute parsing site

`NakedParser` at
`rustc_attr_parsing/src/attributes/codegen_attrs.rs:221-333`.
Registered in the parser table at
`rustc_attr_parsing/src/context.rs:188`.

Mechanism (lines 225-239):

```rust
const ATTRIBUTES: AcceptMapping<Self, S> =
    &[(&[sym::naked], template!(Word), |this, cx, args| {
        if let Err(span) = args.no_args() { ... }
        if let Some(earlier) = this.span { ... warn_unused_duplicate ... }
        else { this.span = Some(cx.attr_span); }
    })];
```

`#[naked]` is a single-segment built-in attribute (`sym::naked`
in the root namespace, not under any tool prefix), with
template `Word` (no arguments). Duplicate attributes warn.
Parser emits `Some(AttributeKind::Naked(span))` in
`finalize(...)` at line 332 once compatibility checks pass
(see Q5 for compatibility-check details).

Allowed-targets list at lines 240-246:

```rust
const ALLOWED_TARGETS: AllowedTargets = AllowedTargets::AllowList(&[
    Allow(Target::Fn),
    Allow(Target::Method(MethodKind::Inherent)),
    Allow(Target::Method(MethodKind::Trait { body: true })),
    Allow(Target::Method(MethodKind::TraitImpl)),
    Warn(Target::MacroCall),
]);
```

`#[naked]` legality is constrained to free functions and
methods (inherent + trait + trait-impl with bodies). Statics,
consts, ADTs, modules etc. all fail at rustc's own attribute
gate -- the bridge does not need to revalidate.

`AttributeKind::Naked(Span)` variant at
`rustc_hir/src/attrs/data_structures.rs:910`. The span is
preserved for diagnostic use.

## Q2 -- Source-of-truth flag

`CodegenFnAttrFlags::NAKED` at
`rustc_middle/src/middle/codegen_fn_attrs.rs:167`
(bit `1 << 2`):

```rust
/// `#[naked]`: an indicator to LLVM that no function prologue/epilogue
/// should be generated.
const NAKED                     = 1 << 2;
```

The parsed `AttributeKind::Naked(span)` is folded into
`CodegenFnAttrs.flags |= CodegenFnAttrFlags::NAKED` at
`rustc_codegen_ssa/src/codegen_attrs.rs:82`:

```rust
AttributeKind::Naked(_) => codegen_fn_attrs.flags |= CodegenFnAttrFlags::NAKED,
```

The `codegen_fn_attrs(did) -> &CodegenFnAttrs` query
(declared in `rustc_middle/src/query/mod.rs`; provider at
`rustc_codegen_ssa/src/codegen_attrs.rs`) is the canonical
per-DefId lookup. Bridge codepath: `tcx.codegen_fn_attrs(did)
.flags.contains(CodegenFnAttrFlags::NAKED)`.

**Shim-side filtering note** at
`rustc_middle/src/middle/codegen_fn_attrs.rs:14-28`:

```rust
pub fn codegen_instance_attrs(self, instance_kind: InstanceKind<'_>)
    -> Cow<'tcx, CodegenFnAttrs>
{
    let mut attrs = Cow::Borrowed(self.codegen_fn_attrs(instance_kind.def_id()));
    if !matches!(instance_kind, InstanceKind::Item(_)) {
        if attrs.flags.contains(CodegenFnAttrFlags::NAKED) {
            attrs.to_mut().flags.remove(CodegenFnAttrFlags::NAKED);
        }
    }
    ...
}
```

`#[naked]` strictly applies to the source-level `Item`. rustc
strips the flag from shim-derived `InstanceKind`s (ReifyShim,
ClosureOnceShim, FnPtrShim, ...). For RegseqModel this is
informational confirmation: the natural recognition point is
Item-level (per-DefId), not Instance-level. Phase 2
(Translation+Synthesis) per stage-12 D-12-5 already walks Items
keyed by DefId.

## Q3 -- MIR body shape under `#[naked]`

`rustc_codegen_ssa/src/mir/naked_asm.rs:30-41`:

```rust
let rustc_middle::mir::TerminatorKind::InlineAsm {
    asm_macro: _,
    template,
    ref operands,
    options,
    line_spans,
    targets: _,
    unwind: _,
} = mir.basic_blocks[START_BLOCK].terminator().kind
else {
    bug!("#[naked] functions should always terminate with an asm! block")
};
```

A `#[naked]` function's MIR body is constrained to **exactly
one basic block (START_BLOCK)** whose **terminator is
`TerminatorKind::InlineAsm`**. The `bug!(...)` path is the
codegen's assertion that this invariant holds; rustc enforces
the body shape upstream (HIR + parsing rules ensure the
function body is `asm!(...)` only). Body locals are skipped at
`rustc_codegen_ssa/src/mir/mod.rs:394-396`:

```rust
let codegen_fn_attrs = bx.tcx().codegen_instance_attrs(fx.instance.def);
if codegen_fn_attrs.flags.contains(CodegenFnAttrFlags::NAKED) {
    return vec![];
}
```

Implication for the bridge: a `#[naked]` Item's
`optimized_mir` body carries no RegseqModel-translatable
content -- it is `InlineAsm`-terminator-only, and the
InlineAsm terminator is already rejected at stage-9 D-9-9
row 8 (§23.6.1). See Q6 for the structural-overlap analysis.

## Q4 -- Codegen-side dispatch

`rustc_codegen_ssa/src/mono_item.rs:44-51` (define pass):

```rust
MonoItem::Fn(instance) => {
    let flags = cx.tcx().codegen_instance_attrs(instance.def).flags;
    if flags.contains(CodegenFnAttrFlags::NAKED) {
        naked_asm::codegen_naked_asm::<Bx::CodegenCx>(cx, instance, item_data);
    } else {
        base::codegen_instance::<Bx>(cx, instance);
    }
}
```

`rustc_codegen_ssa/src/mono_item.rs:74-82` (predefine pass):

```rust
MonoItem::Fn(instance) => {
    let attrs = cx.tcx().codegen_instance_attrs(instance.def);
    if attrs.flags.contains(CodegenFnAttrFlags::NAKED) {
        // do not define this function; it will become a global assembly block
    } else {
        cx.predefine_fn(instance, linkage, visibility, symbol_name);
    };
}
```

`codegen_naked_asm` (at `mir/naked_asm.rs:16-56`) wraps the
function's `asm!(...)` template with a generated
prologue/epilogue (function prefix + label + suffix) and emits
the whole thing as a `codegen_global_asm` call. No defined
function symbol exists post-codegen; the function becomes a
top-level assembly block. `rustc_middle/src/mir/mono.rs:154-159`
forces `InstantiationMode::GloballyShared` for naked Fns
(exactly-once instantiation, same posture as
`contains_extern_indicator`).

**Naked-implies-not-inlined.**
`rustc_codegen_ssa/src/codegen_attrs.rs:327-329` overrides any
`#[inline]` to `InlineAttr::Never` when NAKED is set:

```rust
if codegen_fn_attrs.flags.contains(CodegenFnAttrFlags::NAKED) {
    codegen_fn_attrs.inline = InlineAttr::Never;
}
```

Not load-bearing for RegseqModel (rejection makes inline
treatment moot), but confirms the rustc-level
codegen-shape divergence: a naked function is structurally a
top-level asm block, not a callable codegen function.

## Q5 -- Attribute-target restrictions

`NakedParser::finalize` at
`rustc_attr_parsing/src/attributes/codegen_attrs.rs:248-333`
runs the cross-attribute compatibility pass over
`cx.all_attrs`. The ALLOW_LIST at lines 261-293 enumerates
attributes that may legally co-occur with `#[naked]`:

```rust
const ALLOW_LIST: &[rustc_span::Symbol] = &[
    sym::cfg_trace, sym::cfg_attr_trace,
    sym::test, sym::ignore, sym::should_panic, sym::bench,
    sym::allow, sym::warn, sym::deny, sym::forbid,
    sym::deprecated, sym::must_use,
    sym::cold, sym::export_name, sym::link_section, sym::linkage,
    sym::no_mangle, sym::instruction_set, sym::repr,
    sym::rustc_std_internal_symbol,
    sym::rustc_align, sym::rustc_align_static,
    sym::naked,
    sym::doc,
];
```

Notably absent (incompatible per rustc rules): `#[inline]`
and `#[track_caller]` -- per the comment block at lines
252-258 listing "`#[inline]`, `#[track_caller]`, `#[test]`,
`#[ignore]`, `#[should_panic]`" as incompatible. The
`test` / `ignore` / `should_panic` / `bench` symbols DO
appear in the ALLOW_LIST (`:266-269`) despite that comment,
because rustc admits them at the parser level **so that
`rustc_builtin_macros::test` can produce its own
better-targeted diagnostic** later in the pipeline -- this
is downstream diagnostic-routing, not a substantive
compatibility statement. The substantive ALLOW_LIST membership
that matters for rustc's compatibility check is `#[inline]`
and `#[track_caller]`: both are absent from ALLOW_LIST, so a
function carrying `#[naked]` together with either of those
fails the cross-attribute pass at `:325-329`.

Tool attributes (`#[rseq::*]` family) pass the allow check
unconditionally at line 300:

```rust
if other_attr.segments().next().is_some_and(|i| cx.tools.contains(&i.name)) {
    continue 'outer;  // skip the incompatibility check for tool attrs
}
```

Implication for the bridge: rustc has already enforced
target-kind legality (Q1 ALLOWED_TARGETS) and attribute-
combination legality (Q5 ALLOW_LIST) by the time the bridge
inspects the resulting `CodegenFnAttrs`. The bridge only
needs to make the v1-rejection decision; it does not need to
revalidate which other attributes accompany `#[naked]`.

## Q6 -- Structural overlap with stage-9 D-9-9 row 8

Per Q3, a `#[naked]` function's MIR body has a single
`TerminatorKind::InlineAsm` terminator in the START_BLOCK and
no other content. Stage-9 D-9-9 row 8 (§23.6.1) already
rejects `TerminatorKind::InlineAsm` at terminator translation
with diagnostic:

> "inline assembly is not supported in Regseq driver source;
> use the domain-specific intrinsics (MMIO, synchronization,
> reset, clock, ...) for low-level operations"

So a `#[naked]` Item that survived Item translation would
fail at body translation when the bridge walks the
START_BLOCK terminator. The structural overlap means
**inaction at Item translation is not silently admitting
`#[naked]`** -- the InlineAsm rejection catches it in any
case.

However, **Item-translation rejection is preferable** for
three reasons:

1. **Diagnostic clarity.** The Item-translation diagnostic
   names the construct the source author wrote
   (`#[naked]`) and recommends the eventual replacement
   path (future ISR-stub domain extension). The InlineAsm
   diagnostic names the body shape rustc produced, not the
   source-level attribute the author wrote, which obscures
   the actual blocker.

2. **Fail-fast.** Item translation is Phase 2 per stage-12
   D-12-5; body translation runs later in the same phase
   per the per-Item walk. Rejecting at the Item observation
   point avoids per-block translation work for an Item that
   was never going to succeed.

3. **Spec coherence with D-9-9 row 8.** D-9-9 row 8's
   InlineAsm rejection is general -- any function with an
   InlineAsm terminator anywhere fails. `#[naked]` is the
   specific source-level construct that statistically
   guarantees InlineAsm. Calling out the specific construct
   at the Item level keeps the spec's rejection vocabulary
   discoverable: a driver author searching "naked" in the
   spec finds the rejection row directly, not via the
   generic InlineAsm rejection.

The Item-translation row and the existing Terminator-
translation row are **siblings**, not duplicates: each
catches its own observation surface (attribute presence vs.
body terminator vocabulary). Both rows remain in the spec.

## Synthesis -- what D-14-7 needs to land

Mechanism for the rejection (per Q2): bridge calls
`tcx.codegen_fn_attrs(did).flags.contains(CodegenFnAttrFlags::NAKED)`
during Phase 2 (Translation+Synthesis) per stage-12 D-12-5
when discovering each Item; emits a v1-rejection diagnostic
if the flag is set.

Diagnostic content (per Q6): names `#[naked]` explicitly;
recommends "future `#[rseq::interrupt(...)]` domain
extension that synthesises a C/asm wrapper around an
ordinary Rust handler body" per the deferral framing in
stage-14 `expectations.md` Finding #2.

Rejection-table home: §15.1 already carries the bridge's
construct-rejection catalogue, currently spanning three
phases (type, call, place translation). Adding "Item
translation" as a fourth phase is consistent with §15.1's
existing structure (one row per construct + phase) and
avoids creating a parallel rejection table for one entry.
The §15.1 header note enumerating "three categories" gets
extended to four.

Field removals (per the Finding #2 mechanics):

- `Item.is_naked: bool` field deletion at §8.0 schema (and
  its 6-line `// is_naked corresponds to #[naked]` comment).
- `RegseqModel::instance_is_naked(...)` helper deletion at
  §3.5.2 + helper-inventory count downshift in the narrative
  that follows the helper block.
- "`is_naked` +" trim from the §8.0 schema's field-
  enumeration narrative paragraph.
- §3.6.6 paragraph-3 "D-14-7 territory" wording -> LOCKED-
  state wording.

Pretty-printer edits (per the §5.3 + §5.6 pretty-printer-
snapshot.md surfaces):

- §5.3 per-flag table: delete the `is_naked = true` ->
  `#[naked]` row and the `is_naked = false` -> suppressed
  row.
- §5.6 row-order recap: delete item 3
  (`Item.is_naked == true -> #[naked]`).
- §5.1 atomic-decorator-syntax example list: drop `#[naked]`
  from the atomic-shape examples (the attribute no longer
  surfaces from any field).

Specimen-side overlap with D-14-9 (Finding #4): Specimen 70
currently carries `is_naked: true` in its input and
`#[naked]` in its output. D-14-7 cleans the `is_naked`
side; D-14-9's split into Specimen 70 + 70b handles the
`opaque_handle_eligible` side. Both edits land before
merger close per `expectations.md` "Cross-finding
integration" -- D-14-7 does not need to fully rewrite
Specimen 70 (D-14-9's territory), only to align its
`is_naked` field with the field-deletion outcome.

Cross-stage friction (per Finding #2 mechanics):

- **D-12-1.3 [REOPENED]** -- typed Item-level fields list
  loses the `is_naked: bool` entry.
- **D-9-9 [REOPENED]** -- the 9-row rejection table grows
  by one row (becomes 10 rows or, if the row lives in
  §15.1 instead, the §23.6.1 table count stays at 9 and
  §15.1 gains the new entry; either way, D-9-9's rejection-
  vocabulary surface is extended).
- **D-13-4.3 [REOPENED]** -- per-flag rendering table loses
  the `is_naked` row.

All three REOPENs happen at stage-14 merger close per the
§5.5 merger-contract step 4 mechanic.
