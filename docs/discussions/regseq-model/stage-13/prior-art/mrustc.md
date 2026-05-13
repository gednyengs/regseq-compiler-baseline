# mrustc prior-art: pretty-printing

Primary-source pass against mrustc's MIR/HIR dump for stage 13's
8-D-tag framing. mrustc is a Rust-to-C compiler in C++; cited as
the COUNTER-PRECEDENT to the rustc + rustc_public + Charon
Rust-source-style consensus on D-13-1.

## Sources

- `src/mir/dump.cpp:1-612` -- MIR dump (`MirDumper::dump_mir` +
  `TreeVisitor` over HIR).
- `src/mir/mir.cpp:15-720` -- `operator<<` overloads on
  `Constant`, `LValue`, `Param`, `RValue`, `Terminator`,
  `Statement`.
- `src/mir/mir.hpp:351-720` -- declarations / sigil conventions.
- `src/hir/dump.cpp:240-314` -- HIR-level item rendering
  (`visit_function`, `visit_constant`, `visit_static`).
- `src/hir/type.cpp:15-378` -- `TypeRef::fmt`, `CoreType` /
  `BorrowType` / `ArraySize` operator<<.

## Per-D-tag relevance

### D-13-1 (master rendering style + method shape)

**mrustc has TWO co-existing modes that differ for the same
value.** First, `MirDumper::dump_mir` (mir/dump.cpp:26-239) is
the spec dumper: Rust-source-leaning sigil style (`_$N`, `bb`+N,
`goto bbN`, `if X { goto Y } else { goto Z }`). Second, the
free `operator<<(ostream&, T)` overloads in mir/mir.cpp:15-720
are debug-log style: constructor-form (`Use(X)`, `BinOp(L ADD R)`,
`Switch(val : 0 => bbN, ...)`, `Call(retval = path(args), bbT,
bbE)`). The two render the same value differently. dump.cpp
internally calls a `fmt_val` helper (mir/dump.cpp:240-396) that
RE-RENDERS RValue in a sigil-leaning hybrid (`&mut (X)`, `[X; N]`,
`Struct path { vals }`) instead of delegating to the constructor-
form `operator<<`.

**Implication for D-13-1.** This is the unique mrustc precedent:
splitting "Display impls on IR types" (constructor) from "the
spec dumper" (Rust-source). RegseqModel's stage-13 design is
single-mode: one `PrettyPrinter` owns the canonical text. Stage
13 should NOT adopt mrustc's dual rendering -- a single
`PrettyPrinter` returning `String` is the coherent choice. mrustc's
constructor-mode is reference for debug-log compactness, not for
the canonical surface.

### D-13-2 (body envelope + indentation)

Envelope (mir/dump.cpp:30-234): `let _$N: T;` declarations stacked
above `bb`+N`: { stmts; terminator; }` blocks, framed by the HIR
`TreeVisitor` which emits the `fn $path(...) -> Ret { ... }` outer
shell (mir/dump.cpp:497-537). 3-space INDENT (`"   "` at
mir/dump.cpp:399-401) -- narrower than rustc's 4. Block body uses
nested indent. Drop flags rendered as `let df$N = expr;`
(mir/dump.cpp:34) -- a stage-13 non-concern (no drop flags in
RegseqModel's `optimized_mir` view).

### D-13-3 (span-rendering mode)

**No spans rendered at all** in dump output. Span info reaches
the dumper (e.g. mir/dump.cpp:41, 51, 121 commented-out
`DEBUG(...)` lines) but is gated through C++ DEBUG macros, never
emitted to the user-facing dump. mrustc converges with rustc_public
on "no spans by default" -- another data point for D-13-3 candidate
(a) off-by-default toggle.

### D-13-4 (Item-level rendering shape)

**Source-faithful with attribute-prefix lines** (hir/dump.cpp:245-
311). Function: `[const ][unsafe ][extern "ABI" ]fn $name<args>(
arg$0: T, ...) -> Ret`. `where`-bounds rendered on a separate
indented line (hir/dump.cpp:260-263). Attributes rendered ABOVE
the item: `#[link_name="..."]` for static linkage names
(hir/dump.cpp:292-293). Externs rendered as `extern static $name:
T` (no body) (hir/dump.cpp:303-304). Const: `const $name: T = $val;`
(hir/dump.cpp:312-314). Note this is the same Rust-source attribute
syntax (`#[...]` on its own line) the rustc + rustc_public peers use.

**Linkage for non-name cases.** Linkage variants beyond `link_name`
are NOT rendered in mrustc's HIR dump -- mrustc ignores them. Stage
13's D-13-4 has more surface to cover.

### D-13-5 (`render_ty` per-arm specs)

`TypeRef::fmt` at hir/type.cpp:224-377 is unambiguously
**Rust-source-style** with embedded `/*...*/` debug annotations:

- `Ref` -> `&{lifetime} {mut }T` (333-343)
- `Pointer` -> `*const T` / `*mut T` / `*move T` (344-352)
- `Array` -> `[T; N]` (321)
- `Slice` -> `[T]` (324)
- `Tuple` -> `(T, T, ...,)` always-trailing-comma (327-332)
- `Function` -> `[for<...>] [unsafe ] [extern "ABI" ]fn(T, ...) -> Ret`
  (356-372)
- `Path` (Adt) -> `$path/*S*/` etc., binding kind in inline comment
  (269-279)
- `TraitObject` -> `dyn (Trait+Marker+'lt)` parens-wrapped (283-294)
- `Diverge` -> `!` (263-264)
- Primitives -> source spelling via `CoreType` operator<<
  (type.cpp:21-49).

**Cross-project convergence on type rendering: 4-of-4** (rustc,
rustc_public, Charon, mrustc) all pick Rust-source-style for
`render_ty`. This is the strongest evidence in the prior-art set
for D-13-1 candidate (a) and D-13-5 default-form choice. Even
mrustc -- the "constructor-style" project for MIR values -- picks
Rust-source for types.

### D-13-6 (body-level renderer per-arm specs)

Two competing arms exist for the same vocabulary. dump.cpp's
sigil-leaning `fmt_val` (240-396) is closer to RegseqModel
stage-13 territory:

- `LValue` storage rendered as `retval` / `a$N` (Argument) / `_$N`
  (Local) / `($static)` (mir.cpp:136-153).
- `LValue::Wrapper` projections chain WITHOUT parens (mir.cpp:154-
  171): `Field` -> `.idx`, `Deref` -> `*` (prefix), `Index` ->
  `[_N]`, `Downcast` -> `#idx`. Compare rustc's parenthesised
  `(*X.field)`; mrustc's flat chain is simpler but ambiguous for
  precedence-sensitive readings.
- `Borrow` -> `&[mut |move ](X)` (mir/dump.cpp:280-291).
- `SizedArray` -> `[X; N]` (mir/dump.cpp:275-279).
- Aggregates: `Tuple(...)`, `Array([...])`, `Struct path { vals }`,
  `Variant path #idx { vals }` (mir/dump.cpp:358-394).

`mir.cpp::operator<<` versions of the same RValue arms render
differently (constructor-form: `Tuple(vals)`, `Array(vals)`, etc.),
mir.cpp:311-325. Stage 13 inherits one canonical form -- mrustc's
dual render shows the cost of letting both shapes co-exist.

### D-13-7 (worked-example specimens)

**No snapshot-style fixtures.** mrustc emits dump output as a side
effect of compilation (`MIR_Dump_Fcn` at mir/dump.cpp:607-611)
but does not maintain golden specimens or insta-equivalent
snapshots. Test gating happens at run-time correctness (compiled
binaries match), not at MIR-text level. Stage-13 D-13-7 has no
mrustc precedent.

### D-13-8 (sigil tables / ownership tables)

**No documented sigil convention.** Conventions live inline in
dump.cpp / mir.cpp; no header- or doc-level table. Sigil set:
`_$N` (let-decl), `_N` (in-block local), `arg$N` (argument
form), `bb`+N (block), `df$N` (drop flag). Drift between
`_$N` and `_N` for the same `Local` storage is internal mrustc
concern -- a cautionary tale: stage 13 should pin one literal
form per ID and not let dump-vs-debug drift open back up.

## Key takeaway for stage 13

mrustc's strongest evidence for stage 13 is its TYPE rendering
(Rust-source-style, 4-of-4 cross-project convergence -- D-13-5
candidate is decisively settled). mrustc's MIR VALUE rendering is
the counter-evidence on D-13-1: a project that picked
constructor-style debug-mode + sigil dump-mode discovered they
drift, requiring two parallel rendering surfaces. Stage 13 should
NOT replicate that split; one `PrettyPrinter` returning canonical
text is the coherent choice. mrustc's flat projection chain
(no parens) is a candidate worth considering vs rustc's
pre/post-wrap; the simplification reads cleanly when the chain
is short but loses parse-precedence cues for `Deref(Field)` vs
`Field(Deref)` ambiguities.

## Cross-stage drift / counter-evidence

- **D-13-1: NOT a counter-precedent for the dump form.** mrustc's
  dump-mode (MirDumper) IS Rust-source-leaning -- the constructor-
  form lives in `operator<<` for debug-log purposes. The Rust-
  source-style consensus (rustc + rustc_public + Charon + mrustc-
  dump-mode) is 4-of-4 for the *spec dumper* role.
- **D-13-1: counter-precedent for the dual-render anti-pattern.**
  mrustc's split between MirDumper and operator<< is the artifact
  to avoid; stage 13's single `PrettyPrinter` design is
  load-bearing.
- **D-13-2 INDENT:** mrustc uses 3 spaces vs rustc's 4. No
  consensus across 4 peers (rustc=4, rustc_public=4, Charon=2 per
  stage-3 prior-art, mrustc=3). Stage 13 picks; 4-space matches
  rustc-output familiarity.
- **D-13-6 projection chain:** rustc wraps with parens
  (`pre_fmt_projection` / `post_fmt_projection`); mrustc chains
  flat. Stage 13 picks; flat is simpler, parens are clearer for
  ambiguous chains.
