# Rustc Source Pointers

Files in the pinned rustc-src that are relevant to designing the
MIR/HIR -> RegseqModel construction pass. Skim references, not a
comprehensive study. Each entry says what it is and when to consult
it.

All paths are relative to:
`~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/`

## Discovery / monomorphization collection

### `rustc_monomorphize/src/collector.rs` (~1840 lines)
The reachability walker. Top-level entry is `pub(crate) fn collect_crate_mono_items(tcx, strategy) -> (Vec<MonoItem<'tcx>>, UsageMap<'tcx>)` at line 1803. The module doc-comment (lines 1-200) is the best single explanation of how rustc discovers everything that will be codegen'd: it starts from "roots" (`#[no_mangle]` items, lang items, and other non-generic, non-const callables), then transitively follows mentions through MIR bodies. It distinguishes "used" items (definitely needed) from "mentioned" items (e.g. function pointer references). Drop glue is resolved through `Instance::resolve_drop_in_place` (line 944). Generic instantiation flows through `Instance::resolve_*` calls (lines 790, 967, 1428). The big match on `InstanceKind` at lines 1018-1046 enumerates every kind of monomorphized thing rustc tracks (DropGlue, FutureDropPollShim, AsyncDropGlue, VTableShim, ReifyShim, ClosureOnceShim, FnPtrShim, CloneShim, FnPtrAddrShim, Item, Intrinsic, Virtual, ThreadLocalShim). Read this before designing reachability roots for the construction pass.

### `rustc_monomorphize/src/lib.rs` (~55 lines)
Tiny re-export hub. Worth a glance to see what's exposed at crate boundary.

### `rustc_monomorphize/src/partitioning.rs` (~1345 lines)
Calls `collect_crate_mono_items` (line 1138) and partitions results into codegen units. Mostly irrelevant to us (we don't partition), but useful as the canonical "consumer" of the collector output, showing how `(Vec<MonoItem>, UsageMap)` is actually used downstream.

### `rustc_monomorphize/src/mono_checks/` (abi_check.rs, move_check.rs)
Post-collection sanity passes. Skim only if we want to understand what rustc considers "OK to monomorphize" so we can mirror those checks.

### `rustc_middle/src/ty/mod.rs` line 1671
`pub fn instance_mir(self, instance: ty::InstanceKind<'tcx>) -> &'tcx Body<'tcx>` is the single function the construction pass will call per `Instance` to get a `mir::Body`. It dispatches to `optimized_mir`, shim builders, or intrinsic fallback bodies depending on `InstanceKind`. This is the "fetch the body" primitive.

## Per-body walk pattern

### `rustc_codegen_ssa/src/mir/mod.rs` (~608 lines)
The canonical end-to-end body walk. `pub fn codegen_mir(cx, instance)` at line 173 is the entry. The flow is exactly what RegseqModel construction's per-instance lowering does:
1. `tcx.instance_mir(instance.def)` to fetch the body (line 182).
2. Construct `FunctionCx` (lines 50-156) holding `instance`, `mir`, per-local state, per-block cache.
3. Compute `traversal_order = traversal::mono_reachable_reverse_postorder(...)` (line 250).
4. Allocate locals via `arg_local_refs` then `vars_and_temps_iter` (lines 254-296).
5. For each reachable block, call `fx.codegen_block(bb)` (line 308).

This is the most important reference for writing our own per-body walk.

### `rustc_codegen_ssa/src/mir/block.rs` (~2099 lines)
The biggest file in the body walk. Contains `codegen_block`, `codegen_terminator`, and the giant per-`TerminatorKind` match. Notable rejection points (see Rejection Points table below) live here. Consult when an unfamiliar terminator surfaces during construction-pass design.

### `rustc_codegen_ssa/src/mir/statement.rs` (~145 lines)
Per-`StatementKind` dispatch. Small, readable. Good first-stop reference.

### `rustc_codegen_ssa/src/mir/rvalue.rs` (~1057 lines)
Per-`Rvalue` dispatch (BinaryOp, Cast, Aggregate, Ref, AddressOf, Repeat, Len, Discriminant, etc.). Consult when an unfamiliar rvalue surfaces during construction-pass design.

### `rustc_codegen_ssa/src/mir/operand.rs` (~1161 lines)
`Operand::Copy / Move / Constant` dispatch. `codegen_operand` at line 1038 has the full match. Constants flow through `eval_mir_constant_to_operand` and then the `ConstValue` cases at lines 169-194 (`Slice { alloc_id, meta }`, `Indirect { alloc_id, offset }`, `Scalar`, `ZeroSized`).

### `rustc_codegen_ssa/src/mir/place.rs` (~524 lines)
`codegen_place` at line 324 walks `ProjectionElem` chains. The match (lines 354-400) covers `Deref`, `Field`, `OpaqueCast`, `UnwrapUnsafeBinder`, `Index`, `ConstantIndex` (both `from_end` variants), `Subslice`, `Downcast`. Reference for understanding any place projection variant.

### `rustc_codegen_ssa/src/mir/locals.rs` (~83 lines)
Tiny. Defines `Locals<V>` indexed by `mir::Local`. Useful if the construction pass needs a similar per-local sidecar map.

### `rustc_codegen_ssa/src/mir/analyze.rs` (~395 lines)
Pre-walk analysis (cleanup_kinds, non-SSA local detection). Probably not directly relevant — the construction pass isn't trying to keep things in registers — but useful precedent for "iterate the whole body once before codegen to gather facts".

### `rustc_codegen_ssa/src/base.rs` (~1136 lines)
Top-level codegen orchestration. `pub fn codegen_crate<B>(...)` at line 681 is the crate-wide driver. It collects mono items, partitions into CGUs, and feeds them to the backend. Not a per-body reference, but documents the outer loop the construction pass's driver will resemble.

### `rustc_codegen_ssa/src/mono_item.rs` (~98 lines)
Tiny. The trait `MonoItemExt` defining `define`, `predefine`, `to_raw_string`. Shows the minimal interface a backend exposes per mono item.

## Smaller, more readable backend

### `rustc_codegen_cranelift/src/base.rs` (~1115 lines)
Same shape as `codegen_ssa/src/mir/mod.rs` but vastly more readable because it doesn't go through the `BuilderMethods` trait abstraction. `pub(crate) fn codegen_fn(tcx, ..., instance) -> CodegenedFunction` at line 33 is the entry point. Line 47: `let mir = tcx.instance_mir(instance.def);`. Line 120 calls `codegen_fn_body(&mut fx, start_block)` defined at line 284, which is the literal `for (bb, bb_data) in fx.mir.basic_blocks.iter_enumerated()` loop with per-statement and per-terminator dispatch inline. This is the best reference for what our walk will look like in straightforward Rust.

### `rustc_codegen_cranelift/src/constant.rs` (~651 lines)
Cranelift's flatter version of constant evaluation. Easier to read than `codegen_ssa/src/mir/constant.rs` for understanding `Const::Unevaluated` flow.

### `rustc_codegen_cranelift/src/inline_asm.rs` (~867 lines)
Cranelift's inline-asm codegen. Reference if we want to know what an `asm!` block looks like in MIR after lowering.

## Stable IR extraction analog

### `rustc_public/src/lib.rs` (~319 lines)
Crate root for the new SMIR-replacement extraction layer. Note: in this nightly the old `rustc_smir` crate has been split into **`rustc_public`** (the public-facing IR types) and **`rustc_public_bridge`** (the rustc-internal proxy that does the actual translation). This is exactly the boundary we'll need to draw between the rustc-facing surface of construction and the rest of RegseqModel construction. The doc comment at lines 1-7 calls out that the goal is eventual publication on crates.io, i.e. true `'tcx` independence.

### `rustc_public/src/mir/body.rs` (~1106 lines)
The owned `Body` schema. `pub struct Body { blocks, locals, arg_count, var_debug_info, spread_arg, span }` (line 15). Mirror enums to study: `TerminatorKind` (line 154), `StatementKind` (line 477), `Rvalue` (line 493), `Operand` (line 672), `Place` (line 680). Note these enums are owned (no `'tcx`) and `Serialize`-able. This is the closest existing analog to RegseqModel's body-level IR shape.

### `rustc_public/src/mir/mono.rs` (~297 lines)
`MonoItem { Fn(Instance), Static(StaticDef), GlobalAsm(Opaque) }` (line 14). `Instance::body() -> Option<Body>` (line 53) is the user-facing fetch. `InstanceKind { Item, Intrinsic, Virtual { idx }, Shim }` (line 30) — note this enum collapses rustc's much larger `ty::InstanceKind` into a stable four-variant subset. We'll likely want a similar narrowing for RegseqModel.

### `rustc_public/src/ty.rs` (~1666 lines)
Owned `Ty`, `TyKind`, `RigidTy`, `AdtDef`, `GenericArgs`, etc. The schema for type information that has been disentangled from `'tcx`. Skim the top-level `pub enum`s only.

### `rustc_public/src/visitor.rs` (~224 lines)
Visitor trait over the owned IR. Useful template if we write a RegseqModel visitor.

### `rustc_public/src/compiler_interface.rs` (~882 lines)
The `with(|cx| ...)` dispatch on a thread-local `CompilerCtxt`. This is how owned types look up their underlying rustc state on demand. **This is the central pattern for solving the `'tcx` lifetime escape problem**: opaque IDs in the public types, plus a contextual lookup back into the compiler.

### `rustc_public_bridge/src/lib.rs` (~294 lines)
The rustc-internal half. The key type is `Tables<'tcx, B: Bridge>` (line 45) which holds bidirectional `IndexMap`s between rustc-internal handles (`DefId`, `AllocId`, `ty::Instance<'tcx>`, `mir::Const<'tcx>`, etc.) and the bridge's opaque IDs. Translation populates these tables; the public IR carries opaque IDs that index back through them. **Read this if we're designing how RegseqModel IDs relate to rustc internals during construction.**

### `rustc_public_bridge/src/builder.rs` (~76 lines) and `bridge.rs` (~59 lines)
Tiny support files defining the `Bridge` trait and ID builders. Quick read.

### `rustc_public_bridge/src/context/mod.rs` (~79 lines)
The `CompilerCtxt` struct. Holds a `TyCtxt<'tcx>` plus a mutable `Tables`. This is the analog of what the construction pass's context object will look like.

## Constant resolution

### `rustc_codegen_ssa/src/mir/constant.rs` (~107 lines)
Compact and very readable. `eval_mir_constant` at line 23 is the one-liner: `self.monomorphize(constant.const_).eval(self.cx.tcx(), self.cx.typing_env(), constant.span).expect("erroneous constant missed by mono item collection")`. Note the panic message — by the time codegen runs, the collector has already triggered evaluation of every required const. The `mir::Const::Unevaluated(uv, _)` arm at line 42 plus `eval_unevaluated_mir_constant_to_valtree` (lines 37-61) shows how named associated constants are resolved via `tcx.const_eval_resolve_for_typeck`. Directly relevant to driver-spec associated constants in RegseqModel.

### `rustc_codegen_ssa/src/mir/operand.rs` lines 169-260
Where `ConstValue` is consumed: `Scalar`, `ZeroSized`, `Slice { alloc_id, meta }`, `Indirect { alloc_id, offset }`. Indirect constants are dereferenced via `bx.tcx().global_alloc(alloc_id).unwrap_memory()` (line 183). Reference for when the construction pass encounters non-scalar consts.

## Const rendering (MIR pretty-printer)

Surfaced by the stage-14 D-14-4 evidence pass. Landmarks for `render_const(ConstId, TyId) -> String` per-arm specs across the closed 11-variant `Const` enum.

### `rustc_middle/src/mir/pretty.rs:1851-1989`

`pretty_print_const_value_tcx(tcx, ct, ty, fmt)` is the primary const-rendering entry. Tuple-match on `(ct, ty.kind())` at line 1872 dispatches across `&str` / `&[u8]` / `[u8; N]` byte-string special cases (1874-1898), `Array` / `Tuple` / `Adt` aggregate forms (1907-1966), `Scalar` (1968-1975) and `ZeroSized` over `FnDef` (1976-1982). All-uninit shortcut at 1864-1869 returns `<uninit>`. Verbose-internals shortcut at 1859-1862. Fallback at 1988 is `{ct:?}: {ty}` debug. **The load-bearing API contract: the renderer takes both `ConstValue` and `Ty` -- type-context is required for `Bytes` / `Aggregate` / `ZST` / `NullPtr` / `Reloc` arms.**

### `rustc_middle/src/mir/pretty.rs:1835-1849`

`comma_sep` -- the recursion helper for Aggregate arms. Takes `Vec<(ConstValue, Ty<'tcx>)>` and renders comma-separated. Each child carries its own per-field type (resolved via `try_destructure_mir_constant_for_user_output`). The pair-form `(ConstValue, Ty)` is the recursion shape.

### `rustc_middle/src/mir/pretty.rs:1831-1833`

`pretty_print_byte_str` -- the byte-literal helper. `write!(fmt, "b\"{}\"", byte_str.escape_ascii())` produces forms like `b"hello"` and `b"\x00\x01\x02"`. Consumed at lines 1885, 1896.

### `rustc_middle/src/mir/pretty.rs:1269-1277`

`Display for ConstOperand` -- the operand-level wrapper. Prefixes `const ` unless the const's type is `FnDef`. Stage-13 D-13-6 already pinned a divergence from rustc here: RegseqModel's `render_operand` for `Operand::Constant(id)` does NOT add the `const ` prefix.

### `rustc_middle/src/ty/print/pretty.rs:1706-1849`

`pretty_print_const_scalar` (1706-1717), `pretty_print_const_scalar_ptr` (1719-1774), `pretty_print_const_scalar_int` (1776-1849). Scalar dispatch: pointer scalars route to byte-string / fn-pointer / typed-cast forms; int scalars dispatch on `ty.kind()` for `Bool` / `Float(_)` / `Uint(_)` / `Int(_)` / `Char` / pointer types / "nontrivial scalar" fallback (`transmute(0x...)`).

### `rustc_middle/src/ty/consts/int.rs:51-138`

`impl fmt::Debug for ConstInt` -- the integer-rendering primitive. Alternate-form (`{:#?}`) used by MIR pretty-printing produces underscore-prefixed suffix: `42_i32`, `42_u32`, `42_isize`. Sentinel collapsing for MIN/MAX at exact-bits equality: `i32::MIN`, `i32::MAX`, `u32::MAX`, etc. The `is_ptr_sized_integral` flag drives `_isize` / `_usize` suffix vs width-specific.

### `rustc_apfloat::ieee::{Half, Single, Double, Quad}::Display`

The IEEE-754 decoded float-display primitives consumed by `pretty_print_const_scalar_int` Float arms (lines 1788-1803). Forms: finite `1.5`, `1`, `0`, `-0`; non-finite `inf`, `-inf`, `NaN`. Rustc concatenates `<val><underscore-if-nonfinite><width-suffix>`: `1.5f32`, `inf_f32`, `NaN_f32`.

### `rustc_public/src/mir/pretty.rs:339-345`

`pretty_mir_const` / `pretty_ty_const` -- stable-IR delegation form. Both call back through `with(|cx| cx.{mir,ty}_const_pretty(...))` to rustc-internal pretty-printers rather than reimplementing. **The escape hatch is unavailable to RegseqModel (no `tcx` at render time), so RegseqModel must implement the dispatch self-contained.**

### `rustc_public/src/mir/pretty.rs:397-430`

`pretty_aggregate` -- stable-IR aggregate rendering (for `Rvalue::Aggregate`, not Const, but the dispatch shape is informative). Forms: `Array` -> `[ ... ]`, `Tuple` -> `( ... )`, `Adt::Enum` -> `<def.trimmed_name()>::<variant.name()>(...)`, `Adt::Struct` -> `<variant.name()>(...)`. No 1-tuple trailing-comma treatment (StableMIR FIXME).

## Instance rendering (MIR pretty-printer body envelope + per-MonoItem dispatch)

Surfaced by the stage-14 D-14-5 evidence pass. Landmarks for `render_instance(InstanceId) -> String` per-variant specs across the closed 6-variant `InstanceKind` enum. Full per-question writeup at `stage-14/prior-art/rustc-instance.md`.

### `rustc_middle/src/mir/pretty.rs:317-369`

`write_mir_pretty(tcx, single, w)` -- top-level entry; walks `dump_mir_def_ids(tcx, single) = tcx.mir_keys(())` (body-bearing items only -- foreign items are absent). Inserts blank-line separators between items. Per-DefId: trivial-const fast path renders `const PATH: TY = const VAL;` one-liner; otherwise `tcx.instance_mir(InstanceKind::Item(did))` -> `MirWriter::write_mir_fn`. `is_const_fn` renders optimized_mir + ctfe_mir back-to-back.

### `rustc_middle/src/mir/pretty.rs:384-399`

`MirWriter::write_mir_fn(body, w)` -- per-body envelope. Calls `write_mir_intro` (sig + `{` + scope tree + coverage info), then per-block `write_basic_block`, then closing `}`, then `write_allocations`. This is the canonical body envelope shape RegseqModel's `pretty-printer-snapshot.md` §3.1 mirrors.

### `rustc_middle/src/mir/pretty.rs:539-572`

`write_mir_intro(tcx, body, w, options)` -- the `<sig> {` opener + scope-tree block (var_debug_info + let-decls + nested scopes via `write_scope_tree:403-521`) + coverage info. Closes with a blank line before the first block. Locals indexed `1..=arg_count` are skipped at the scope-tree level (they're rendered in the sig instead).

### `rustc_middle/src/mir/pretty.rs:612-674`

`write_mir_sig(tcx, body, w)` -- the signature emitter. Dispatches on `DefKind`: `Const | AssocConst` -> `const ` prefix; `Static { mutability: Not }` -> `static ` prefix; `Static { mutability: Mut }` -> `static mut ` prefix; promoteds -> `const ` prefix; `Fn | AssocFn | Ctor | SyntheticCoroutineBody` or `is_closure_like` -> `fn ` prefix; `AnonConst | InlineConst | GlobalAsm` -> no prefix. For function-like: `fn PATH(_1: ArgTy, ...) -> RetTy`. For static / const: `static [mut ]PATH: TY =` or `const PATH: TY =`. **Does NOT emit `unsafe` / `extern "ABI"` keywords on the signature line** -- rustc's MIR pretty-printer omits Safety + FnAbi decoration entirely. The keyword block is `<visibility> [const ] [async ] [unsafe ] [extern "ABI" ] fn` in Rust source grammar, but `write_mir_sig` emits just `fn`.

### `rustc_codegen_ssa/src/mono_item.rs:29-87`

`MonoItemExt::define` + `predefine` -- per-MonoItem dispatch at codegen. Three variants: `MonoItem::Fn(instance)` -> `base::codegen_instance` (the `codegen_mir` path) or `naked_asm::codegen_naked_asm` for `CodegenFnAttrFlags::NAKED`; `MonoItem::Static(def_id)` -> `cx.codegen_static`; `MonoItem::GlobalAsm(item_id)` -> `base::codegen_global_asm`. Predefine pass declares symbols (skipping naked Fns + GlobalAsm). **RegseqModel's `InstanceKind` is a 6-variant superset** of this 3-variant dispatch: it adds Struct / Enum / Union as Instances because RegseqModel emits C typedefs; rustc/LLVM emit nominal types implicitly at use sites. ADT-variant rendering has no rustc precedent.

### `rustc_public/src/mir/mono.rs:14` + `:178-180`

`pub enum MonoItem { Fn(Instance), Static(StaticDef), GlobalAsm(Opaque) }` (line 14). `Instance::emit_mir(self, w)` at lines 178-180: `if let Some(body) = self.body() { body.dump(w, &self.name()) } else { Ok(()) }`. **Body-bearing-only: foreign instances emit nothing**. RegseqModel cannot mirror this -- foreign declarations are emission units and must render as `[...]fn PATH(args) -> Ret;` declaration form.

### `rustc_public/src/mir/pretty.rs:38-88`

`function_body(writer, body, name)` -- stable-IR body envelope. Header `fn name(args) -> Ret {`; locals decl block `let mut _N: T;` / `let _N: T;` (skips arg locals); `debug NAME => _N;` lines; per-block `bbN: { stmts; terminator }`. **Caller passes `name`** -- the body doesn't know its identifier. The composer (RegseqModel's `render_instance`) supplies the path. Always uses `fn` keyword (no `static` / `const` envelope path because rustc_public's `Static` variant has no `body()`).

## Top-level render (MIR pretty-printer batch entry + iteration order)

Surfaced by the stage-14 D-14-6 evidence pass. Landmarks for the top-level `render(&self) -> String` composer renderer per stage-13 D-13-4.6. Full per-question writeup at `stage-14/prior-art/rustc-render.md`.

### `rustc_middle/src/mir/pretty.rs:317-369`

`write_mir_pretty(tcx, single, w)` -- top-level batch entry. Shape: 3-line WARNING + HINT comment header + `for def_id in dump_mir_def_ids(tcx, single)` loop with **first-flag-guarded blank-line separator between items** (no leading blank before first item; no trailing blank after last item). Per-DefId dispatch is trivial-const fast path OR `write_mir_fn(body, w)` (covered above). `single: Option<DefId>` parameter filters to one body; `None` means walk all `mir_keys`. **RegseqModel's `render(&self)` is the no-filter analog** -- single-Instance dispatch is already covered by D-14-5's `render_instance(InstanceId)`.

### `rustc_mir_transform/src/lib.rs:328-359`

`fn mir_keys(tcx) -> FxIndexSet<LocalDefId>` -- collects body-owners from `tcx.hir_body_owners()` (HIR-walk source order), filters out `GlobalAsm`, appends coroutine-by-move bodies + tuple-struct ctors at the end. `FxIndexSet` preserves insertion order. **Iteration order is HIR-walk source-declaration order, NOT content-keyed sort.** RegseqModel's `model.instances` is strictly more deterministic: F4-2 sorted by `RegseqInstanceKey` per `spec.md` §16.3, with rank-grouping baked in (Functions before Statics before Consts before Structs/Enums/Unions per §9.3 `regseq_instance_kind_rank`). `render(&self)` walks `instances.iter_enumerated()` in native order; no re-sort.

### `rustc_middle/src/mir/mono.rs:508-574`

`MonoItem::items_in_deterministic_order` -- codegen-time per-CGU sort with **two modes** gated on `tcx.sess.opts.unstable_opts.codegen_source_order` (`mono.rs:553`). **Default**: `items.sort_by_cached_key(|&(i, _)| i.symbol_name(tcx))` at `mono.rs:571` -- symbol-name only. **`-Z codegen-source-order`**: `(Option<Span>, SymbolName)` `ItemSortKey` at `mono.rs:514-550` -- span first, symbol-name tiebreak. Per-CGU, not per-crate. **Supporting evidence for D-14-6.2's iteration-order choice**, not load-bearing -- rustc's own consensus is "sort upstream; don't re-sort at every consumer," with the upstream sort key varying by mode.

### `rustc_public/src/mir/pretty.rs` (entire file)

**NO top-level batch renderer.** Stable-IR has `function_body` (per-body envelope, above) + `Instance::emit_mir(&self, w)` at `mono.rs:178-180` (per-Instance); no analog of `write_mir_pretty`. Stable-IR consumers write their own iteration loop over `crate.collect_mono_items()`. **Confirms RegseqModel's `render(&self)` top-level composer is genuinely RegseqModel-new at the stable-public-IR tier**, mirroring D-14-5's ADT-variant rendering finding (no rustc precedent for nominal-type rendering at the Instance level either).

## `#[naked]` handling

Surfaced by the stage-14 D-14-7 evidence pass. Landmarks for the `#[naked]` rejection at Item-translation per external-review Finding #2. Full per-question writeup at `stage-14/prior-art/rustc-naked.md`.

### `rustc_attr_parsing/src/attributes/codegen_attrs.rs:221-333`

`NakedParser` -- attribute parser for `#[naked]`. Single-segment `sym::naked` (no tool prefix); template `Word` (no arguments); allowed targets `Fn` + `Method(Inherent|Trait{body:true}|TraitImpl)`. Emits `AttributeKind::Naked(Span)` at line 332. `finalize(...)` runs the ALLOW_LIST cross-attribute compatibility pass at lines 261-293; tool attributes (`#[rseq::*]`) pass the allow check unconditionally at line 300. **rustc enforces target-kind + attribute-combination legality before the bridge observes the result.**

### `rustc_middle/src/middle/codegen_fn_attrs.rs:167`

`CodegenFnAttrFlags::NAKED = 1 << 2` -- the source-of-truth flag bit. Folded from `AttributeKind::Naked(_)` at `rustc_codegen_ssa/src/codegen_attrs.rs:82`. **Bridge codepath: `tcx.codegen_fn_attrs(did).flags.contains(CodegenFnAttrFlags::NAKED)` is the canonical per-DefId lookup.** Same source-of-truth bit rustc's own codegen consults at `rustc_codegen_ssa/src/mono_item.rs:46`.

### `rustc_middle/src/middle/codegen_fn_attrs.rs:14-28`

`codegen_instance_attrs(self, instance_kind) -> Cow<'tcx, CodegenFnAttrs>` -- strips the NAKED flag for non-Item `InstanceKind`s (ReifyShim, ClosureOnceShim, FnPtrShim, etc.). Confirms `#[naked]` is strictly source-level per-Item; shim-derived Instances never carry it. Recognition point is therefore Item-level (per-DefId), not Instance-level -- aligned with stage-12 D-12-5 Phase 2 (Translation+Synthesis) Item-walk.

### `rustc_codegen_ssa/src/mir/naked_asm.rs:30-41`

`codegen_naked_asm` asserts the MIR body's `START_BLOCK` terminator is `TerminatorKind::InlineAsm` (single-block + InlineAsm-terminator-only body shape). The `bug!("#[naked] functions should always terminate with an asm! block")` path at line 40 is the upstream contract -- rustc enforces this body shape at HIR-lowering / parsing. **Structural overlap with stage-9 D-9-9 row 8** (`TerminatorKind::InlineAsm` rejection at §23.6.1): a `#[naked]` Item that survived Item translation would fail at body translation when the bridge walks the terminator. Item-translation rejection is preferable for diagnostic clarity, fail-fast, and spec-vocabulary discoverability.

### `rustc_codegen_ssa/src/mono_item.rs:44-51` + `:74-82`

Codegen-side dispatch: `MonoItem::Fn(instance)` with `CodegenFnAttrFlags::NAKED` set dispatches to `naked_asm::codegen_naked_asm` (define pass) and is skipped by `predefine_fn` (predefine pass) -- "the function becomes a global assembly block, not a defined symbol." `rustc_middle/src/mir/mono.rs:154-159` forces `InstantiationMode::GloballyShared` for naked Fns. `rustc_codegen_ssa/src/codegen_attrs.rs:327-329` overrides any `#[inline]` to `InlineAttr::Never` when NAKED is set. **Confirms `#[naked]` is structurally a top-level asm block at codegen, not a callable function** -- RegseqModel's v1 rejection forecloses this whole codegen-shape divergence.

## Rejection points

| Construct | File:line | What rustc does |
|---|---|---|
| `asm!` (cranelift path) | `rustc_codegen_cranelift/src/base.rs:523` | Matches `TerminatorKind::InlineAsm`, dispatches to `crate::inline_asm::codegen_inline_asm_terminator`. Rejects `MAY_UNWIND` at `:532` with "cranelift doesn't support unwinding from inline assembly." |
| `asm!` (ssa path) | `rustc_codegen_ssa/src/mir/block.rs:1557` | Matches `TerminatorKind::InlineAsm`, dispatches to `self.codegen_asm_terminator(...)`. No outright rejection at the IR level; backends decide. |
| naked `asm!` (whole-fn) | `rustc_codegen_ssa/src/mir/naked_asm.rs:30` | Asserts the body's only terminator is `TerminatorKind::InlineAsm` and emits the asm directly. |
| Coroutine ops (Yield, CoroutineDrop) - ssa | `rustc_codegen_ssa/src/mir/block.rs:1550-1552` | `bug!("coroutine ops in codegen")` — coroutine transform is supposed to have already eliminated these. |
| Coroutine ops - cranelift | `rustc_codegen_cranelift/src/base.rs:579-583` | `bug!("shouldn't exist at codegen {:?}", bb_data.terminator())` for `Yield`, `FalseEdge`, `FalseUnwind`, `CoroutineDrop`. |
| `FalseEdge` / `FalseUnwind` (borrowck artifacts) | `rustc_codegen_ssa/src/mir/block.rs:1553-1555` | `bug!("borrowck false edges in codegen")`. |
| Async drop leakage | `rustc_codegen_ssa/src/mir/block.rs:585-589` | `assert!(async_fut.is_none() && drop.is_none(), "Async Drop must be expanded or reset to sync before codegen")`. |
| Unwinding under `panic = "abort"` | `rustc_mir_transform/src/abort_unwinding_calls.rs:47` | The `AbortUnwindingCalls` MIR pass rewrites unwind edges to `UnwindAction::Unreachable` when `tcx.sess.panic_strategy() == PanicStrategy::Abort`. By the time we read `optimized_mir`, panic=abort builds have already had cleanup paths removed. See also `rustc_mir_transform/src/remove_noop_landing_pads.rs:15` and `rustc_mir_transform/src/ffi_unwind_calls.rs:140`. |
| Coroutine transform (panic=abort interaction) | `rustc_mir_transform/src/coroutine.rs:1175` | `if !tcx.sess.panic_strategy().unwinds() { ... }` — coroutine state machines skip unwind paths when configured for abort. |

The pattern: rustc's late stages (codegen, post-collection) use `bug!` and `assert!` for "this MIR shape should never reach me." The construction pass should use `unsupported_construct!`-style errors instead so the driver author sees a useful message, but the *placement* of the checks should mirror what we see here.

## Tool-namespace attributes

Surfaced by the stage-14 D-14-1 evidence pass. Landmarks for Q7 intrinsic recognition (`#[rseq::intrinsic("NAME")]`) and the DomainAttr surface.

### `rustc_middle/src/ty/mod.rs:1707-1758`

`tcx.get_attrs` (single segment), `tcx.get_all_attrs`, `tcx.get_attrs_by_path` (multi-segment via `Attribute::path_matches`), and `has_attr` / `has_attrs_with_path`. **`get_attrs_by_path(did, &[sym::rseq, sym::intrinsic])` is the primary API for stage 14.**

### `rustc_hir/src/hir.rs:1184-1268`

`AttrArgs` (`Empty` / `Delimited(DelimArgs)` / `Eq`), `AttrPath { segments: Box<[Symbol]>, span }`, `AttrItem`, and `Attribute = Parsed(AttributeKind) | Unparsed(Box<AttrItem>)`. **Custom tool attributes always land in `Unparsed`** (doc at 1265-1267).

### `rustc_hir/src/hir.rs:1303-1395`

`AttributeExt` impl. `path_matches` (1351) matches segment sequences exactly on `Unparsed`; `meta_item_list` (1313) returns `Some(ThinVec<MetaItemInner>)` for `Delimited` args only.

### `rustc_hir/src/attrs/mod.rs:34-54`

`find_attr!` macro. **Only matches `Attribute::Parsed`** -- skips `Unparsed`. Unusable for tool attributes despite `get_all_attrs`' doc recommending it.

### `rustc_ast/src/ast.rs:597-605` + `2250-2253`

`MetaItemInner = MetaItem | Lit(MetaItemLit)`; `LitKind::Str(Symbol, StrStyle)`. The `meta_item_list()` -> `MetaItemInner::Lit` -> `LitKind::Str(name, _)` chain extracts `"NAME"`.

### `rustc_ast/src/attr/mod.rs:672-677`

`MetaItemInner::lit() -> Option<&MetaItemLit>`. The accessor for `Delimited("NAME")` payloads.

### `rustc_middle/src/ty/util.rs:1685-1701`

`intrinsic_raw` provider. Gates on `tcx.has_attr(def_id, sym::rustc_intrinsic)` (1686) -- single-segment built-in only. **Verifies external-review Finding #1: wrong hook for tool attributes.**

### `rustc_middle/src/query/mod.rs:182-186` + `2244-2248`

Queries: `registered_tools(()) -> &'tcx RegisteredTools` and `intrinsic_raw(DefId) -> Option<IntrinsicDef>`.

### `rustc_resolve/src/macros.rs:123-160`

`registered_tools` provider. Walks `#![register_tool(NAME)]` (133); predefined tools added at 155-158. `RegisteredTools = FxIndexSet<Ident>` per `rustc_lint_defs/src/lib.rs:831`.

### `rustc_resolve/src/ident.rs:691-694` + `1896-1904`

Tool-prefix resolution: registered tool -> `Res::ToolMod` via `ToolPrelude`; multi-segment path rewrites to `Res::NonMacroAttr(NonMacroAttrKind::Tool)` and stops. **Unregistered tools fail at lookup** -- rustc emits an unresolved-path error. **Practical consequence: bridge does not need to re-check `tcx.registered_tools(())` for `#[rseq::*]` it sees in MIR.**

## When to consult

- Encountering an unfamiliar MIR pattern -> `rustc_codegen_cranelift/src/base.rs:284` (`codegen_fn_body`) for the linear walk; then the relevant `rustc_codegen_ssa/src/mir/{block,statement,rvalue,operand,place}.rs` for variant-by-variant detail.
- Designing reachability roots for the construction pass -> `rustc_monomorphize/src/collector.rs` doc comment (lines 1-200) and `collect_crate_mono_items` at line 1803.
- Writing the per-body walk -> `rustc_codegen_ssa/src/mir/mod.rs:173` (`codegen_mir`) and the cranelift mirror at `rustc_codegen_cranelift/src/base.rs:33`.
- Looking up an Operand/Place variant's intent -> `rustc_codegen_ssa/src/mir/operand.rs:1038` (`codegen_operand`) and `rustc_codegen_ssa/src/mir/place.rs:324` (`codegen_place`).
- Resolving named/associated constants -> `rustc_codegen_ssa/src/mir/constant.rs` (entire file, ~107 lines).
- Designing the `'tcx` escape boundary -> `rustc_public_bridge/src/lib.rs:45` (`Tables<'tcx, B>`) plus `rustc_public/src/compiler_interface.rs` (the `with(|cx| ...)` pattern).
- Choosing a stable subset of `InstanceKind` -> `rustc_public/src/mir/mono.rs:30` (the four-variant collapse).
- Locating where to emit our own rejections -> the Rejection points table above; reuse those file:line landmarks as templates.

## What's NOT in this inventory

- `rustc_codegen_llvm/`. Too large, too LLVM-specific. The `rustc_codegen_ssa` + `rustc_codegen_cranelift` pair already covers the body-walk pattern.
- `rustc_const_eval/` (CTFE interpreter). Different domain — we trust the collector's pre-evaluation and consume the resulting `ConstValue`.
- `rustc_expand/` (macro expansion). Pre-MIR; we consume post-expansion HIR/MIR.
- `rustc_borrowck/` (borrow checker). Runs before MIR construction; we don't redo its work.
- `rustc_mir_transform/` as a whole. Only the specific files relevant to panic=abort behaviour are cited above. The pinned pipeline runs at `mir_opt_level=0`, so most transforms there will not have run on the bodies we read; the abort-specific ones are special because they're correctness-required, not optimization passes.
- `rustc_hir_typeck/`, `rustc_trait_selection/`, `rustc_traits/`. We consume already-typechecked HIR/MIR.
