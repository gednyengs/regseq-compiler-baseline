# Stage 13 -- Discussion Record

Distilled record of the design dialogue producing the
stage-13 pretty-printer spec. Each entry captures the
question raised, options considered, the decision, the
reasoning chain, methodology rules invoked, prior-art
evidence, and cross-references to
`pretty-printer-snapshot.md` (the new top-level spec
file's draft, per D-13-9) and `spec-snapshot.md`
(working copy of `../spec.md` for the small in-place
edits per D-13-9.2). Round-by-round Codex audit trail
lives in `review-log.md` (created on first review round).

Status of decisions in this file mirrors
`pretty-printer-snapshot.md` (the bulk pretty-printer
spec) plus minimal in-place edits to `spec-snapshot.md`
at §15 / §18 / §19.8 / §20.6 / §21.8 / §22.8 / §23.9 /
§24.9 / §25.x (stub + cross-references per D-13-9.2)
and `../README.md` at §4 / §5.5 step 5 / §5.9 rule 6
(methodology extension per D-13-9.3).

D-tag convention: prefixed `D-13-K` form per
`../README.md` §5.2.

Status tags:
- `[LOCKED]` -- audit-shape recorded; per-stage Phase A
  closed; ready for Phase B Codex review.
- `[CLOSED-BY-ABSORPTION]` -- subsumed into a
  later D-tag during Phase A; retained for history.
- `[REOPENED]` -- a later sub-decision or sibling D-tag
  forced a revision; superseded entry kept for history.

Phase A closed 2026-05-06 with all 8 main D-tags +
41 sub-decisions LOCKED + 1 D-tag closed-by-absorption
(D-13-8 → D-13-9.6) across a single combined cluster.

Two mid-cluster substantive threads surfaced during
discussion:
1. The **building-block-vs-composer architectural
   division** (D-13-4.6) -- prompted by the user's
   question "shouldn't we be rendering Instances
   instead of Items?"; resolved by clarifying that
   stage 13 owns NEW rendering BUILDING BLOCKS
   (`render_ty`, `render_item`, body-level renderers),
   inherited building blocks (`render_def_path`
   stage 12 per D-12-6, `render_def_path_value` +
   `render_span` stage 1) are consumed but not
   stage-13-owned, and stage 14 owns COMPOSERS
   (`render_const` per D-4-K, `render_instance`,
   top-level `render`). `render_item` stays as a
   public stage-13 API entry per D-12-K.1.
2. The **pretty-printer spec location reframe**
   (D-13-9 emergent) -- prompted by the user's
   suggestion "could we create a separate
   pretty-print.md instead of adding to spec.md?";
   resolved by promoting it to a new D-13-9, locking
   the full pretty-printer spec into a top-level
   companion file `pretty-printer.md` (matching the
   project's existing `architecture.md` / `decisions.md`
   / `regseq-background.md` / `rustc-references.md`
   side-file pattern). The methodology rule extension
   to README §5.9 / §5.5 / §4 is part of D-13-9.3.

The reframe under D-13-9 absorbs D-13-8 (sigil-table
review + ownership-table consolidation): both land as
sections inside the new `pretty-printer.md` (§3 sigils,
§8 ownership table) rather than as in-place edits to
`spec.md` §18.

---

## 1. Decision discussions

### D-13-1 -- Master rendering style + method shape [LOCKED]

**Question.** Stage 13's broadened scope (per stage-11
D-11-K + stage-12 D-12-K cascades) covers 20 inbound
items as a coherent rendering-decision surface. Stage 13
must lock the master rendering style ONCE for the whole
surface (D-11-K's load-bearing rationale) -- the textual
form (Rust-source-style vs constructor-style vs
C-source-like) and the method shape (context-bearing
printer vs free fns vs Display impls).

**Options considered.**

Aspect A1 (textual rendering style):
- (a) Rust-source-style with sigil decorations (`&'a mut Vec<u8>`).
  4-peer prior-art convergence: rustc-internal
  `mir/pretty.rs` + `ty/print/pretty.rs`; rustc_public
  `mir/pretty.rs`; Charon `pretty/fmt_with_ctx.rs`;
  project's own `def_path.rs` / `render_def_path_value`
  precedent. Stage-3 D14 lean preserved as input.
- (b) Constructor-style (`Ref(Region("a"), Mut, Adt(Vec, [u8]))`).
  mrustc `operator<<` debug-log path
  (`prior-art/mrustc.md`) -- counter-precedent.
- (c) C-source-like declarator (`Vec<u8> *`). Awkward
  for `dyn`, generics, lifetimes. No project precedent.
- (d) Hybrid (Rust-source types, MIR-form bodies).
  No primary precedent.

Aspect A2 (method shape):
- (a) Methods on `PrettyPrinter<'a>` returning `String`.
  Already shipped at stage 1
  (`crates/regseq-model/src/pretty.rs`); Q5 pinned
  context-bearing printer (IDs need `&self.model`
  lookup).
- (b) Free `pretty_X(&mut W: io::Write, ...)` fns.
  rustc_public structural pattern; closest external
  analog but RegseqModel already shipped (a).
- (c) `Display` / `Debug` impls on IR types.
  rustc-internal pattern. Incompatible with Q5
  (printer needs context for ID resolution).

**Decision.** **(a) + (a)**: Rust-source-style with
sigil decorations rendered via `PrettyPrinter<'a>`
methods returning `String`.

**Reasoning.**
- A1 (a) has 4-peer prior-art convergence: rustc-internal,
  rustc_public, Charon, and the project's own
  `render_def_path_value` precedent (`crate::module::Item`,
  `{closure#0}`) all commit to Rust-source-style. mrustc's
  dual-rendering split (sigil-style for MIR dumps +
  constructor-form for debug logs; the same value
  rendering two different ways) is the explicit
  anti-pattern that motivates picking ONE style.
- A2 (a) is structurally mandated by Q5 (context required
  for ID resolution) and already shipped. A2 (c) is
  ruled out by Q5; A2 (b) is the closest external
  analog (rustc_public's structural pattern) but
  RegseqModel's shipped methods-on-PrettyPrinter
  shape is the natural carry-forward. Per-method
  dispatch is lifted from rustc_public's free-fn
  pattern at the API-shape level (one method per
  public renderer, per-arm internal match) without
  changing return type.
- The synthesis takes rustc-internal's textual choices
  and rustc_public's structural pattern (per-method
  dispatch on `PrettyPrinter` vs Debug-impls), yielding
  a coherent surface continuous with the project's
  existing scaffold.

**Methodology rules invoked.**
- *Compositional behavior beats special-case flags* --
  the master style applies uniformly; per-renderer style
  dissent is rejected by construction.
- *Spec is mutable* -- stage-3 D14 deferred the renderer-
  style call to stage 13; stage 13 makes the call with
  fully-populated inputs (FnSig per stage 11; Item /
  DefPath per stage 12).
- *Snapshot stability* (stage-13 §2 principle) --
  the choice subordinates per-renderer freedom to
  whole-surface coherence so snapshot tests remain
  stable when individual renderers refine.

**Prior-art evidence.**
- `prior-art/rustc.md:23-49` (Per-D-tag relevance D-13-1):
  rustc-internal hybrid Rust-source-style with sigil
  decorations; primary-source citations at
  `rustc_middle/src/mir/pretty.rs:1083-1180` (Rvalue
  arms), `:1251-1260` (Operand),
  `rustc_middle/src/ty/print/pretty.rs:712`
  (`pretty_print_type` entry).
- `prior-art/rustc_public.md:18-44` (D-13-1): same
  Rust-source-style; free `pretty_X` fns on `&mut W:
  io::Write` -- closest structural analog;
  primary-source citations at `rustc_public/src/mir/
  pretty.rs:38, 90, 128, 171, 204, 229, 326, 347, 397`.
- `prior-art/charon.md:30-42` (D-13-1): Rust-source-style
  via AstFormatter trait + FmtWithCtx trait + WithCtx
  wrapper threading `FmtCtx { translated, generics,
  locals, indent_level }` context. Primary source in
  `charon/charon/src/pretty/fmt_with_ctx.rs`.
- `prior-art/mrustc.md:23-44` (D-13-1): counter-precedent
  -- `MirDumper` emits Rust-source-leaning sigil-style
  for MIR dumps but parallel `operator<<` overloads
  emit constructor-form for debug logs (dual-rendering
  split anti-pattern). Primary sources in
  `mrustc/src/mir/dump.cpp` and `mrustc/src/mir/mir.cpp`.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §2 (Master rendering
  style) -- the canonical lock for the whole surface.
- `pretty-printer-snapshot.md` §1 (Mission and consumer
  model) -- the consumer-facing rationale.

**Sub-decisions.**

#### D-13-1.1 -- Textual style: Rust-source-style with sigil decorations [LOCKED]

**Decision.** Rust-source-style with sigil decorations
per A1 (a). Reads continuously for engineers familiar
with rustc's `--emit=mir` output and the Rust-source
language; coherent with project's existing
`render_def_path_value` precedent.

#### D-13-1.2 -- Method shape: PrettyPrinter methods returning String [LOCKED]

**Decision.** Methods on `PrettyPrinter<'a>` returning
`String`. Existing scaffold preserved; Q5 context-required
constraint respected. v2 `&mut String` overload is
non-breaking extension if hot-path consumer surfaces;
no v1 evidence demands it.

#### D-13-1.3 -- Stage-3 D14 closes by absorption [LOCKED]

**Decision.** Stage-3 D14 (renderer-style call) closes
by absorption into D-13-1. The historical D14 entry in
stage-3 discussions (preserved as deferred-decision
context) is referenced as preserved-input rationale;
D-13-1's Phase B audit-shape capture cites it. At stage-13
merger close, D14's status flips to
`[CLOSED-BY-ABSORPTION]` in `decisions.md` with
annotation `(absorbed into D-13-1 per stage 13)`.

---

### D-13-2 -- Body envelope + indentation + sigil placement [LOCKED]

**Question.** Per D-13-1's master Rust-source-style,
the body envelope shape and indentation policy must lock
once for the entire body-level rendering surface. Stage
13 also wires the `_N` (LocalId) and `bbN` (BlockId)
sigil rendering sites at first consumer per stage-1 +
D-10-K.2 + D-11-K. The envelope's boundary with
`render_item` (D-13-4 Item-level shape) needs explicit
naming.

**Options considered.**

Aspect B1 (body envelope shape):
- (a) rustc-converged: `fn $path(_1: ArgTy, _2: ArgTy)
  -> RetTy { let mut _0: RetTy; let _3: T; ...; bbN: {
  stmt; stmt; terminator -> [edges]; } }`. rustc-internal
  `mir/pretty.rs:539, :612, :714`; rustc_public
  `mir/pretty.rs:38`; Charon precedent.
- (b) Implicit locals (omit declarations; infer types
  at use sites). No primary precedent; loses type
  information needed for `place_ty` walks at snapshot
  stability.
- (c) Body-only without function header. Splits envelope
  ownership across D-13-2 + D-13-4; incoherent.

Aspect B2 (indentation):
- (a) 4-space INDENT, 8-space stmt indent (rustc-converged
  `INDENT = "    "` at `mir/pretty.rs:19`).
- (b) 2-space (project Rust style elsewhere; not
  pretty-printing precedent).
- (c) Configurable. Adds surface area without evidence.

Aspect B3 (sigil wirings):
- (a) Inline at first consumer; `format!("_{}", id.index())`
  for `LocalId`, `format!("bb{}", id.index())` for
  `BlockId`. rustc + rustc_public both inline these in
  Local / BasicBlock Debug impls.
- (b) Dedicated helper methods on `PrettyPrinter`. Adds
  API surface for textually trivial output.

**Decision.** **B1 (a) + B2 (a) + B3 (a)**: rustc-converged
envelope, 4-space INDENT (8-space stmt indent inside
`bbN: {}`), inline sigil wiring at first consumer.

**Reasoning.**
- B1 (a) preserves type information that snapshot
  tests + `place_ty` walks need; rustc + rustc_public
  + Charon converge on this envelope so RegseqModel
  pretty-print output reads continuously for engineers
  familiar with `--emit=mir`.
- B2 (a) is direct rustc adoption; matches `--emit=mir`
  baseline indentation.
- B3 (a) is the §2 "Compositional behavior beats
  special-case flags" principle (no parallel
  `render_local_sigil` / `render_block_sigil` helpers
  duplicating trivial output).

**Methodology rules invoked.**
- *Compositional behavior beats special-case flags*
  (B3 sigil inlining).
- *Compiler-theory-first* (envelope and indentation
  inherited from rustc + rustc_public converged
  evidence rather than re-derived).
- *C semantics with information preservation* --
  type info on local declarations preserves what
  rustc gives us for free at consumer sites.

**Prior-art evidence.**
- `prior-art/rustc.md:51-67` (D-13-2 envelope +
  indentation): primary-source citations at
  `rustc_middle/src/mir/pretty.rs:539
  (write_mir_intro)`, `:612 (write_mir_sig)`,
  `:714 (write_basic_block)`, `:19 (INDENT)`,
  `:21 (ALIGN = 40)`.
- `prior-art/rustc_public.md:46-55` (D-13-2): same
  envelope; primary-source citations at
  `rustc_public/src/mir/pretty.rs:38 (function_body)`,
  `:91 (statement INDENT)`, `:173 (terminator INDENT)`.
- `prior-art/charon.md:44-52` (D-13-2): AstFormatter
  threads `indent_level` in `FmtCtx`; same envelope
  shape. Primary source in
  `charon/charon/src/pretty/fmt_with_ctx.rs`.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §3 (Body envelope,
  indentation, sigil conventions; absorbs §18.1
  sigil table per D-13-9).

**Sub-decisions.**

#### D-13-2.1 -- Body envelope shape [LOCKED]

**Decision.** rustc-converged envelope `fn $path(...)
-> Ret { let _N: T; ...; bbN: { stmt; stmt; terminator
-> [edges]; } }`.

#### D-13-2.2 -- Indentation = 4-space INDENT, 8-space stmt indent [LOCKED]

**Decision.** `INDENT = "    "` (4 spaces); statements
inside `bbN: {}` rendered at 8-space indent (envelope's
4 + INDENT step 4).

#### D-13-2.3 -- Sigil wirings inline at first consumer [LOCKED]

**Decision.** No dedicated `render_local_sigil` /
`render_block_sigil` methods. `LocalId` renders as
`format!("_{}", id.index())` at `render_place` call
sites + locals declaration block. `BlockId` renders as
`format!("bb{}", id.index())` at `render_basic_block`
header + successor labels in `render_terminator`.

#### D-13-2.4 -- D-13-2 / D-13-4 boundary [LOCKED]

**Decision.** Function header `fn $path(...) -> Ret`
(plus all Item-level attribute prefixes per the D-12-K
bundle: linkage, inline_hint, link_section, is_unsafe,
is_naked, lang-item flags, opaque_handle_eligible)
belongs to **D-13-4** (Item-level shape). D-13-2 owns
**body content only** -- the `{ let _N: T; ...; bbN: {
... } }` portion. D-13-4's `render_item` composes
attribute block + kind-keyword + path; the per-Instance
body content (inside `{ }`) is composed by stage-14
`render_instance` which delegates to D-13-2's envelope
and the body-level renderers.

#### D-13-2.5 -- Locals block formatting [LOCKED]

**Decision.** One declaration per line. `let mut _N:
<render_ty(T)>;` for mutable locals (per stage-11 D-11-2
`LocalDecl.mutability == Mutability::Mut`); `let _N:
<render_ty(T)>;` for immutable. RETURN_PLACE always
emits as `let mut _0: <ret_ty>;` (rustc-converged --
return place is logically mutable since the body
assigns to it). Debug name (per stage-11 D-11-2
`LocalDecl.debug_name: Option<StringId>`) renders as
standalone `debug NAME => _N;` lines AFTER the locals
declarations block, gated by D-13-3.4
`include_debug_names`.

---

### D-13-3 -- Span-rendering mode (and debuginfo gating) [LOCKED]

**Question.** Spans (per stage-1 / `Span` schema) and
debug names (per stage-11 D-11-2
`LocalDecl.debug_name`) are debuginfo classes that
inflate snapshot tests when included. Stage-13 §2
principle ("Snapshot stability is the load-bearing
consumer") rejects always-on rendering. The choices:
default mode, toggle mechanism (since rustc's compile-
time `-Z` flag pattern doesn't translate), span shape
when toggle on, debug-name shape when toggle on,
synthetic-span suppression policy.

**Options considered.**

C1 (default mode):
- (a) Default-off, opt-in via toggle (rustc-internal
  pattern; `mir/pretty.rs:51-58`
  `PrettyPrintMirOptions.include_extra_comments` gated
  by `-Z mir-include-spans`).
- (b) Always-off, no toggle infrastructure (rustc_public
  pattern; `mir/pretty.rs` ships zero span content).
- (c) Always-on (no primary precedent; rejected per §2
  snapshot stability principle).

C2 (toggle mechanism):
- (a) `PrettyPrinter` option-field set at construction
  via `with_options(model, opts)` constructor; existing
  `new(model)` keeps signature, defaults all-off.
- (b) Per-method opt-in flag.
- (c) Thread-local context (rustc_public-style
  `with(|cx| ...)`).

C3 (span shape when toggle on):
- (a) Trailing inline `// <render_span>` aligned to
  col 40 (rustc-internal `mir/pretty.rs:21 ALIGN = 40`;
  `:736-751` call sites).
- (b) Out-of-line span block at body end.
- (c) Inline non-aligned.

C4 (debug-name shape when toggle on):
- (a) Standalone `debug NAME => _N;` lines after
  locals declarations, gated by separate
  `include_debug_names: bool` field
  (rustc `--emit=mir`-converged + finer control).
- (b) Same flag as spans (`include_extra_comments`-style
  umbrella per rustc).
- (c) Always-on (debug-name rendering is deterministic
  per-source, but variable across rustc nightly versions
  if `var_debug_info` extraction changes).

C5 (synthetic-span suppression):
- (a) Suppress when `SourceFileId::SYNTHESIZED`
  (no trailing comment for spans on synthesised
  constructs).
- (b) Render literal `<synthesized>:1:1: 1:1` for
  uniformity.

**Decision.** **C1 (a) + C2 (a) + C3 (a) + C4 (a) +
C5 (a)**:
- Default-off, opt-in via `PrettyPrintOptions`
  option-field constructed via
  `PrettyPrinter::with_options(model, opts)`.
- Existing `PrettyPrinter::new(model)` keeps signature,
  defaults options to all-off.
- When `include_spans=true`: trailing inline
  `// <render_span>` aligned to col 40. Spans on
  synthesised constructs (`SourceFileId::SYNTHESIZED`)
  suppress (no trailing comment).
- When `include_debug_names=true`: standalone
  `debug NAME => _N;` lines after locals declarations.
- `PrettyPrintOptions` shape: `{ include_spans: bool,
  include_debug_names: bool }` with implicit `Default`
  impl (all-off).

**Reasoning.**
- C1 (a) subsumes C1 (b) at zero cost: the toggle
  infrastructure is a few lines, and snapshot tests
  default-off keeps insta baselines compact while
  diagnostic / debug consumers can opt in.
- C2 (a) is the idiomatic Rust approach for the
  rustc compile-time-flag pattern; backwards-compatible
  (existing constructor preserved); testable; no hidden
  state.
- C3 (a) inherits rustc-internal's exact format directly
  (the `render_span` method already shipped at stage 1
  produces `path:l:c: l:c` matching rustc's
  `span_to_diagnostic_string` shape).
- C4 (a) separates spans from debug names because
  their snapshot-stability profiles differ: debug
  names from `var_debug_info` can drift across rustc
  versions even on stable source. Finer control at
  minimal cost (one extra bool field).
- C5 (a) keeps synthetic-Item snapshots clean; the
  literal `<synthesized>:1:1: 1:1` carries no positional
  information so suppression doesn't lose data, and
  synthetic spans are bridge-introduced (vtable shims,
  drop glue, ConstMemoryStatic) -- they would otherwise
  proliferate noise in test fixtures involving
  bridge synthesis.

**Methodology rules invoked.**
- *Snapshot stability* (stage-13 §2 principle) --
  default-off keeps insta baselines compact and
  unaffected by debuginfo drift.
- *Compositional behavior beats special-case flags* --
  the toggle is the single mechanism (option-field +
  default impl); no parallel "is_debug_name_field" /
  "skip_synthetic_spans" flag at per-renderer sites.

**Prior-art evidence.**
- `prior-art/rustc.md:69-87` (D-13-3): primary-source
  citations at `rustc_middle/src/mir/pretty.rs:51-58
  (PrettyPrintMirOptions.include_extra_comments)`,
  `:736-751 (call sites)`, `:21 (ALIGN = 40)`.
- `prior-art/rustc_public.md:57-67` (D-13-3): zero span
  content at all (rustc_public's `mir/pretty.rs` ships
  no span output), evidencing the always-off candidate.
- `crates/regseq-model/src/pretty.rs:67-80`:
  `render_span` already produces
  `path:lo_line:lo_col: hi_line:hi_col` form; D-13-3
  wires the gating layer on top.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §4 (Span-rendering and
  debuginfo modes).

**Sub-decisions.**

#### D-13-3.1 -- Default mode: default-off, opt-in via toggle [LOCKED]

#### D-13-3.2 -- Toggle mechanism: PrettyPrintOptions struct + with_options constructor [LOCKED]

`PrettyPrintOptions { include_spans: bool,
include_debug_names: bool }` with implicit `Default`
impl producing all-false. Constructor:
`PrettyPrinter::with_options(model, opts)` returns
`PrettyPrinter<'a>`. Existing `PrettyPrinter::new(model)`
preserved unchanged (delegates to `with_options(model,
PrettyPrintOptions::default())`).

#### D-13-3.3 -- Span shape (when toggle on) [LOCKED]

Trailing inline aligned to col 40: `<stmt>; // <render_span>`.
Direct rustc adoption.

#### D-13-3.4 -- Debug-name shape (when toggle on) [LOCKED]

Standalone `debug NAME => _N;` lines AFTER locals
declarations block. Per-stage-11 D-11-2's optional
`LocalDecl.debug_name: Option<StringId>` -- only locals
with `Some(name)` contribute debug lines; locals with
`None` are silent.

#### D-13-3.5 -- Synthetic-span suppression [LOCKED]

When `include_spans=true`, spans on `SourceFileId::SYNTHESIZED`
suppress (no trailing comment). Synthetic constructs
(vtable shims, drop glue, ConstMemoryStatic per stage-13
§13 + §6 family) thus render clean even with the toggle
on.

---

### D-13-4 -- Item-level rendering shape [LOCKED]

**Question.** Per D-12-K's 7-entry bundle (`render_item`,
ItemKind variant rendering, Linkage rendering, InlineHint
rendering, Function-flag + lang-item-flag rendering,
link_section rendering, opaque_handle_eligible rendering),
stage 13 must lock the Item-level rendering shape: the
attribute-decorator syntax, the default-suppression
policy, the per-flag rendering form (rustc source-faithful
where the rustc attribute exists), the per-ItemKind
variant rendering, and the `render_item` envelope
formatting.

The user's mid-cluster question "shouldn't we be
rendering Instances instead of Items?" surfaced an
architectural division concern, captured as
sub-decision D-13-4.6.

**Options considered.**

E1 (attribute syntax):
- (a) Rust-source `#[attr]` / `#[attr = "..."]` /
  `#[attr(...)]` form. Direct D-13-1 consequence.
- (b) C-style `[attr=value]`. Clashes with D-13-1.
- (c) Bare key=value. No precedent.

E2 (default-suppression):
- (a) Suppress all default/false-valued fields.
  Stage-13 §2 snapshot-stability principle.
- (b) Always render every field. Predictable but
  noisy; defeats §2.
- (c) Tunable via `PrettyPrintOptions`. No v1 evidence.

E3 (per-flag rendering): per the F-3 table (see
Decision below).

E4 (per-ItemKind rendering): kind-keyword + DefPath
form, per the F-4 table (see Decision below).

E5 (`render_item` envelope formatting):
- (a) Multi-line when ≥1 attribute renders, one per
  line; kind-keyword + path on final line. One-line
  bare `kind $path` when zero attributes survive
  suppression.
- (b) Width-driven (one-line when total ≤ 80 cols).
- (c) Per-call mode flag.

**Decision.** **E1 (a) + E2 (a) + E3 (table) + E4
(table) + E5 (a)**:
- All attribute prefixes use Rust-source `#[...]` form.
- All default-valued fields suppress (Linkage::Default,
  InlineHint::None, link_section: None, is_unsafe:
  false, is_naked: false, lang-item flags all-false,
  opaque_handle_eligible: false).
- Per-flag rendering: see table in §1.4.3 of
  `pretty-printer-snapshot.md` -- rustc source-faithful
  forms (`#[no_mangle]`, `#[inline(always)]`,
  `#[link_section = "..."]`, `#[lang = "..."]`,
  `#[naked]`); RegseqModel-internal
  `#[opaque_handle]` for the bridge-outcome flag.
  `is_unsafe = true` rendered as `unsafe` keyword
  prefix on `fn` (Rust-source modifier keyword form).
- Per-ItemKind kind-keyword + DefPath: `[unsafe ]fn
  $path` / `static [mut ]$path` / `const $path` /
  `struct $path` / `enum $path` / `union $path`.
- Multi-line layout when ≥1 attribute renders (one
  attribute per line); one-line bare `kind $path`
  when zero attributes survive suppression.

**Reasoning.**
- E1 (a) is direct D-13-1 inheritance.
- E2 (a) is the §2 snapshot-stability principle:
  default-valued fields rendering would tickle every
  snapshot when bridge populates a new field default.
  Common case is the cleanest case.
- E3's per-flag rendering favours rustc source-faithful
  forms because they're the most readable and continue
  the project's `render_def_path_value` precedent of
  matching what rustc renders. The one bridge-outcome
  flag (opaque_handle_eligible) gets a RegseqModel-
  internal annotation form `#[opaque_handle]` (no
  rustc source equivalent). The
  `unsafe` keyword (not attribute) for `is_unsafe`
  matches rustc source modifier convention; only `fn`
  accepts the keyword (statics aren't unsafe; `unsafe
  static` is non-syntactic).
- E4 mirrors rustc's HIR pretty-printing: kind-keyword
  + path is the source-level wrapper. ADT layouts and
  function signatures live on Instance per Option B,
  so render_item shows pre-mono Item-level data only.
- E5 (a) is a simple rule with no width parameter;
  multi-line for instrumented Items keeps snapshot
  diffs stable when an attribute is added/removed.
  One-line for bare Items keeps the common case
  compact.

**Methodology rules invoked.**
- *Compositional behavior beats special-case flags* --
  default-suppression falls out of "render only if
  non-default"; no parallel `should_render: bool`
  per Item.
- *C semantics with information preservation* --
  Item-level metadata (linkage, inline hint, link
  section, is_unsafe / is_naked, lang-item flags,
  opaque_handle_eligible) is the source-level alphabet
  RegseqModel consumes for C codegen; the rendering
  preserves it visibly when set, suppresses when
  default.
- *Snapshot stability* (§2 principle).

**Prior-art evidence.**
- `prior-art/rustc.md:89-101` (D-13-4): rustc HIR +
  MIR pretty-printer's attribute prefix conventions
  (`#[no_mangle]`, `unsafe fn`, `#[inline(always)]`);
  primary-source pattern at `rustc_middle/src/mir/
  pretty.rs:612 (write_mir_sig)` for the function-
  header form; HIR attribute-printing infrastructure
  (general rustc precedent; not single-line citable).
- `prior-art/rustc_public.md:69-79` (D-13-4): doesn't
  directly cover Item-level rendering (renders bodies,
  not Items via `mir/pretty.rs`); serves as evidence
  that Item-level rendering shape has no fixed
  external precedent beyond rustc-source-faithful
  patterns.
- `prior-art/charon.md:64-80` (D-13-4): Charon's
  Item rendering uses Rust-source-faithful attribute
  forms via the FmtCtx + WithCtx wrapper. Primary
  source in `charon/charon/src/pretty/fmt_with_ctx.rs`.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §5 (Item-level rendering
  shape).

**Sub-decisions.**

#### D-13-4.1 -- Attribute syntax = Rust-source `#[...]` form [LOCKED]

#### D-13-4.2 -- Default-suppression for all default/false fields [LOCKED]

#### D-13-4.3 -- Per-flag rendering table [LOCKED]

| Field | Rendered form |
|-------|---------------|
| `linkage = NoMangle` | `#[no_mangle]` |
| `linkage = Foreign` | `#[foreign]` |
| `linkage = Weak` | `#[linkage = "weak"]` |
| `linkage = Default` | suppressed |
| `inline_hint = Hint` | `#[inline]` |
| `inline_hint = Always` | `#[inline(always)]` |
| `inline_hint = Never` | `#[inline(never)]` |
| `inline_hint = None` | suppressed |
| `is_unsafe = true` | `unsafe` keyword on kind-keyword (`unsafe fn`) |
| `is_unsafe = false` | suppressed |
| `is_naked = true` | `#[naked]` |
| `is_naked = false` | suppressed |
| `is_phantom_data = true` | `#[lang = "phantom_data"]` |
| `is_unsafe_cell = true` | `#[lang = "unsafe_cell"]` |
| `is_manually_drop = true` | `#[lang = "manually_drop"]` |
| any lang-flag = false | suppressed |
| `link_section = Some("...")` | `#[link_section = "..."]` |
| `link_section = None` | suppressed |
| `opaque_handle_eligible = true` | `#[opaque_handle]` |
| `opaque_handle_eligible = false` | suppressed |

#### D-13-4.4 -- Per-ItemKind variant rendering [LOCKED]

| ItemKind | Rendered form |
|----------|---------------|
| `Function` | `[#[attrs]\n][unsafe ]fn $path` |
| `Static { mutability: Not }` | `[#[attrs]\n]static $path` |
| `Static { mutability: Mut }` | `[#[attrs]\n]static mut $path` |
| `Const` | `[#[attrs]\n]const $path` |
| `Struct` | `[#[attrs]\n]struct $path` |
| `Enum` | `[#[attrs]\n]enum $path` |
| `Union` | `[#[attrs]\n]union $path` |

`$path` resolves through `render_def_path(ItemId)`
(stage-12 D-12-6; cached at `Item.def_path.rendered`).

#### D-13-4.5 -- render_item envelope formatting [LOCKED]

Always multi-line when ≥1 attribute renders, one
attribute per line; kind-keyword + path on final line.
One-line bare `kind $path` when zero attributes survive
suppression.

#### D-13-4.6 -- Building-block-vs-composer architectural division [LOCKED]

**Question (mid-cluster surfacing; user-prompted).**
Should `render_item` be the headline pretty-printer
entry, or should `render_instance` be?

**Decision.** The architectural division is:
- **Stage 13 owns NEW rendering BUILDING BLOCKS:**
  `render_ty`, `render_item`, all body-level renderers
  (D-11-K bundle).
- **Inherited building blocks** (consumed by composers
  but not stage-13-owned): `render_def_path` (stage 12
  per D-12-6), `render_def_path_value` + `render_span`
  (stage 1, shipped).
- **Stage 14 owns rendering COMPOSERS:** `render_const`
  (D-4-K), `render_instance` (composes `render_item`'s
  wrapper output with per-Instance content -- FnSig +
  body + layout + init + value), top-level `render`
  (iterates Instances, calls `render_instance`).

`render_item` stays a **public** API entry per
D-12-K.1; it's a useful Item-table summary view ("list
all Items in the model"), AND the building block that
`render_instance` composes. Making it private (option
b) would lose the Item-table summary use case.
Moving its ownership to stage 14 (option c) would
require reopening stage-12 D-12-K -- heavyweight for a
naming question.

**Reasoning.** The headline consumer is `render_instance`
(per Q2 eager mono baking, codegen iterates Instances),
but stage 13's role IS the building blocks because:
(i) Items are stage-12-territory schema; rendering style
for the wrapper is naturally stage-13 master-style
territory; (ii) D-11-K's "rendering style ONCE for the
whole surface" rationale extends compositionally to
Item-level rendering (D-12-K compositionally extending
D-11-K is precisely this principle); (iii) Stage 14
inherits D-13-1 master style without re-litigating.

A typical Function Instance rendering composes:
```
[render_item output]: attribute block + `[unsafe ]fn $path`  (stage 13, D-13-4)
+ [render_ty over FnSig]: `(_1: T1, _2: T2) -> Ret`           (stage 13, D-13-5)
+ [body envelope + body-level renderers]: `{ let _N: T; bbN: { ... } }`  (stage 13, D-13-2 + D-13-6)
```

`render_item` produces only the first row; `render_instance`
(stage 14) composes all three.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §1 (Mission and consumer
  model -- the architectural division);
  `pretty-printer-snapshot.md` §5 (Item-level rendering
  shape).

---

### D-13-5 -- `render_ty` per-arm specifications [LOCKED]

**Question.** Stage 3's closed 19-variant `Ty` enum
(`spec.md:2999-3019`) is the surface `render_ty` covers
under D-13-1 master Rust-source-style. Stage 13 must
specify per-variant rendering for each of the 19 variants,
the `Ty::Dynamic` rendering (which integrates the
synthesised vtable's DefPath plus auto-traits), the
`render_generic_args` helper (used by `FnDef`, `Closure`,
`Struct`, `Enum`, `Union`), and the carve-outs deferred
from D-13-1 (lifetime suppression, default-value
suppression in composites).

**Options considered.** Per-variant rendering forms have
strong rustc + Charon convergence; the genuine choices
concentrate at `Ty::Dynamic` (F2) and `render_generic_args`
(F3). See `pretty-printer-snapshot.md` §6 for the
canonical per-arm table.

F2 (`Ty::Dynamic` rendering):
- (a) `dyn <render_def_path(vtable_item)> + AutoTrait
  + AutoTrait`. Vtable Item's DefPath encodes the trait
  + impl per stage-3 §13.4.
- (b) `dyn vtable@<render_ty(vtable_ty)>` (treats
  vtable opaque; loses auto-trait readability).
- (c) Decode principal trait's name from vtable Item's
  DefPath and render `dyn <Trait> + Auto + Auto`
  (cleanest output but requires DefPath-segment
  decoding fragile to §13.4 encoding changes).

F3 (`render_generic_args` form):
- (a) `<T1, T2, K1>` Rust-source turbofish-tail form;
  empty `args` renders empty string. rustc-converged.
- (b) Always show `<>` even when empty. Clutters
  non-generic items.
- (c) `::<T1, T2, K1>` turbofish prefix. Rustc uses
  turbofish only in expression position; type position
  uses bare `<...>`.

**Decision.** **F1 (per-variant table) + F2 (a) +
F3 (a) + F4 (lifetime carve-out non-issue + suppress
empty composites)**:
- 19 per-variant render specs per `pretty-printer-
  snapshot.md` §6.
- `Ty::Dynamic` renders `dyn <render_def_path(vtable_item)>
  + AutoTrait + AutoTrait` with auto-traits in
  stage-3 §10.1 sorted order; empty `auto_traits: []`
  omits the `+` suffix.
- `render_generic_args` produces bare `<...>` non-empty,
  empty string when args empty. Per-arg dispatch:
  `Type` → `render_ty`; `Const` → `render_const`
  (forward-call to stage-14 D-4-K); `Lifetime` →
  none (Q2 region erasure).
- Lifetime suppression non-issue: RegseqModel's
  `Ty::Ref` carries `Provenance` (shared/unique borrow
  flavor), NOT rustc-style lifetime regions per
  stage-3 §10.1.
- Default-value suppression for empty composites:
  empty `auto_traits: []` (no `+` suffix); empty
  `GenericArgs` (no `<>`); `Tuple { fields: [] }` →
  `()`.

**Reasoning.**
- F1 directly adopts rustc-internal `ty/print/pretty.rs`
  per-arm precedent for the 19 variants (most are
  mechanical: `bool`, `i32`, `[T; N]`, `&T`, etc.);
  the Rust-source-style is preserved continuously.
  `FnDef` is rendered as rustc-faithful curly-brace
  form `fn(<arg_tys>) -> <ret_ty> {<path><args>}` --
  Rust-source-faithful is impossible (FnDef is not
  expressible as a type expression in stable Rust)
  so curly-brace rustc-internal form is adopted.
  The Instance lookup for `(item, args) -> InstanceId
  -> InstanceKind::Function.fn_sig` is fine since
  `&self.model` is available to the printer.
- F2 (a) is the natural rendering given RegseqModel's
  `Ty::Dynamic` schema: `vtable_ty: TyId` resolves
  to the synthesised vtable struct's Item, whose
  DefPath encodes the trait + impl. Decoding (option
  c) is fragile -- §13.4 encoding could change.
  Treating opaque (option b) loses readability.
- F3 (a) is rustc-converged Rust-source form.
- F4's lifetime carve-out resolves trivially because
  RegseqModel doesn't carry lifetime regions per
  Q2 + stage-3 §10.1; nothing to suppress.

**Methodology rules invoked.**
- *Compiler-theory-first* (per-arm forms inherited
  from rustc + Charon convergent evidence).
- *Compositional behavior beats special-case flags*
  (default-value suppression of empty composites
  falls out of conditional-render logic; no parallel
  `has_args: bool` flags).
- *C semantics with information preservation*
  (rendering preserves the schema's distinguishing
  shapes -- e.g., `&T` vs `&mut T`, `*const T` vs
  `*mut T`).

**Prior-art evidence.**
- `prior-art/rustc.md:103-129` (D-13-5): per-arm
  coverage; primary-source citations at
  `rustc_middle/src/ty/print/pretty.rs:712
  (pretty_print_type entry)`, `:1037 (Array)`,
  `:1044 (Slice)`, `:737 (Tuple arity-1)`,
  `:796 (Adt defers to print_def_path)`,
  `:763 (FnPtr defers to sig_tys)`,
  `:797 (Dynamic)`, `:736 (Never)`, `:810 (Foreign)`.
- `prior-art/charon.md:82-97` (D-13-5): AstFormatter
  approach with per-Ty-variant arms; same Rust-source-
  style; primary sources in
  `charon/charon/src/pretty/fmt_with_ctx.rs`.
- `prior-art/rustc_public.md:81-89` (D-13-5): punts
  type rendering to the thread-local context
  (`with(|cx| ...)` callback) -- the public Ty's Debug
  impl invokes the rustc-internal type printer via
  the bridge.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §6 (`render_ty` per-arm
  specs).

**Sub-decisions.**

#### D-13-5.1 -- Per-variant rendering table [LOCKED]

19 variants per `pretty-printer-snapshot.md` §6.

#### D-13-5.2 -- Ty::Dynamic = `dyn <vtable-DefPath> + Auto + Auto` [LOCKED]

Auto-traits in stage-3 §10.1 sorted order; empty
`auto_traits` omits the `+` suffix. Vtable Item's
DefPath (`<regseq_synth>::vtable::Trait::for::<ImplTy>`
form per §13.4) encodes the principal trait + impl.

#### D-13-5.3 -- render_generic_args helper [LOCKED]

Form `<T1, T2, K1>` non-empty, empty string when args
empty. `GenericArg` is the closed 2-variant enum
`{ Type(TyId), Const(ConstId) }` per `spec.md` §9.1
(lifetimes erased post-monomorphisation; not
represented). Per-arg dispatch: `GenericArg::Type(ty)`
→ `render_ty(ty)`; `GenericArg::Const(c)` →
`render_const(c)` (forward-call to stage-14 D-4-K).

#### D-13-5.4 -- Lifetime suppression carve-out (non-issue) [LOCKED]

RegseqModel's `Ty::Ref` carries `Provenance` (borrow
flavor), NOT rustc-style lifetime regions per
stage-3 §10.1. So `&T` for SharedBorrow / `&mut T` for
UniqueBorrow renders without any `'a` token. The
carve-out deferred from D-13-1.4 resolves trivially
here.

#### D-13-5.5 -- Forward-call to stage-14 render_const [LOCKED]

For `GenericArg::Const(c)` arms, `render_ty` calls
`self.render_const(c)` -- which is stage-14-owned per
D-4-K. The forward-call is the same pattern stage 4
already used (deferred `render_const` to stage 14
with similar forward references). At stage-14 close
the forward-call resolves to the implementation; at
stage-13 spec time it's a documented dependency.

#### D-13-5.6 -- GenericArgs structural verification (Phase B audit) [VERIFIED]

Phase B Codex review R1 verified `GenericArgs` shape
against `spec.md` §9.1 (line 1828): the enum is the
closed 2-variant `{ Type(TyId), Const(ConstId) }`. No
`Lifetime` arm exists; the defensive
"render-as-nothing" arm initially drafted in §6.7 /
D-13-5.3 was removed at R1. Lifetimes are erased at
the bridge per Q2 region erasure and never appear in
`GenericArgs`.

---

### D-13-6 -- Body-level renderer per-arm specifications [LOCKED]

**Question.** Per stage-11 D-11-K bundle, stage 13
owns per-arm rendering for 9 public renderers
(`render_place`, `render_operand`, `render_rvalue`,
`render_statement`, `render_terminator`,
`render_call_callee`, `render_switch_targets`,
`render_assert_kind`, `render_basic_block`) plus 1
internal helper (`render_terminator_kind`). The
underlying vocabularies are stage-5 through stage-10
closed (PlaceElem, Operand, Rvalue, Statement,
TerminatorKind, BasicBlock).

Most arms are rustc-converged direct adoptions; five
genuine choices concentrate the design surface (G1
projection chain shape; G2 Constant operand wrapping;
G3-Q1 CastKind disambiguation; G3-Q2 AggregateKind
RawPtr arm; G5 render_call_callee Direct arm).

**Decision.** **Mass adoption of rustc-converged forms
+ five sub-decisions** per `pretty-printer-snapshot.md`
§7. Concretely:
- G1 (`render_place`): rustc pre/post-wrap projection
  chain pattern. Field/variant name resolution by name
  when resolvable (via place_ty walk +
  `LayoutKind::Struct/Union/Enum`-active-variant /
  Tuple `fields[idx].name`). Both `FieldLayout.name`
  and `VariantLayout.name` are non-optional `StringId`
  per `spec.md` §11; missing-name is invalid IR
  rejected by validation, not papered over at render
  time per `../README.md` §2 "no defensive fallbacks".
- G2 (`render_operand`): `Constant(id)` renders bare
  `<render_const(id)>` (rustc-converged); rejects
  Charon's `const (...)` explicit wrapper.
- G3 (`render_rvalue`): 9-variant table per the spec
  + AggregateKind 7-variant sub-table; CastKind shown
  only when ambiguous (rustc-converged minimal form;
  ambiguous variants `PointerCoercion(ReifyFnPointer
  | UnsafeFnPointer | ClosureFnPointer | Unsize)` plus
  `Subtype`); RawPtr aggregate rendered as
  `(<op_addr>, <op_meta>) as *const/mut <pointee_ty>`
  (two-operand form per `spec.md` §21.3.6 schema;
  both operand positions required even for thin
  pointers).
- G4 (`render_statement`): 3-variant table;
  `SetDiscriminant` renders variant name from
  `place_ty(p)`'s `LayoutKind::Enum.shape`
  (`VariantLayout.name` non-optional per `spec.md`
  §11).
- G5 (`render_terminator` + sub-renderers): 6-variant
  TerminatorKind table; `render_call_callee.Direct(id)`
  resolves Instance, renders
  `<render_def_path(instance.item)><render_generic_args(instance.generic_args)>`
  (Instance-keyed `generic_args` per `spec.md` §9.2);
  `render_assert_kind` direct adoption of
  rustc_public's 8-template `pretty_assert_message`
  patterns.
- G6 (`render_basic_block`): `bbN: { ... }` envelope,
  4-/8-space indentation, trailing `;` per-statement,
  no trailing `;` on terminator.

**Reasoning.**
- G1's rustc pre/post-wrap projection chain pattern is
  chosen over rustc_public's simpler post-only chain
  because the per-projection wrapping (`Deref` →
  `(*X)`; `Field` → `(X.<name>: <ty>)`; `Downcast` →
  `(X as <variant>)`) preserves the projection
  semantics visibly. Field/variant name resolution
  via the §11 layout schema; both name fields are
  non-optional `StringId` so missing-name is invalid
  IR rejected by validation per the "no defensive
  fallbacks" methodology rule.
- G2's bare `Constant` rendering inherits the rustc
  + project precedent; the const value's rendering
  via `render_const` already encodes its const-ness
  (`42i32`, `true`, `&"..."`).
- G3's CastKind disambiguation policy (b) is the
  rustc-converged minimal form; snapshot stability
  prefers minimal output. Ambiguous-only annotation
  by rule (§7.3.1) keeps the visible-cast set fixed
  to the variants where source / target types fail
  to disambiguate.
- G3-Q2's RawPtr aggregate renders as
  `(<op_addr>, <op_meta>) as *const/mut <pointee_ty>`
  (two-operand form). The schema requires both operand
  positions per `spec.md` §21.3.6 (mirroring rustc's
  `RawPtr(Ty<'tcx>, Mutability)` shape per
  `rustc_middle/src/mir/syntax.rs:1561-1566`);
  thin pointers carry the unit value `()` in operand
  position 1.
- G4's `SetDiscriminant` variant-name rendering reads
  `discriminant(p) = <variant_name>` (over rustc's
  numeric form `discriminant(p) = <variant_idx>`) for
  diagnostic clarity; the place_ty walk to resolve the
  name is already needed for `Downcast` projections.
  `VariantLayout.name` is non-optional `StringId` per
  `spec.md` §11, so this is always resolvable.
- G5's `Direct(InstanceId)` rendering via Instance
  lookup (`render_def_path(instance.item)` +
  `render_generic_args(instance.generic_args)`) is
  consistent with `Ty::FnDef` per D-13-5.1 (both
  resolve through Instance-keyed `generic_args` to
  render path + args).
  `render_assert_kind`'s direct adoption of
  rustc_public's `pretty_assert_message` template
  patterns is the explicit precedent (`prior-art/
  rustc_public.md`).
- G6's envelope inherits stage-10 D-10-1 / D-10-K.1
  shape (`bbN: { stmt; stmt; terminator; }`); the
  4-/8-space indentation matches D-13-2.2.

**Methodology rules invoked.**
- *Compiler-theory-first* (per-arm forms inherited
  from rustc + rustc_public + Charon convergent
  evidence; G3-Q2 RawPtr two-operand form locked at
  R1 against `spec.md` §21.3.6 schema).
- *Compositional behavior beats special-case flags*
  (sigil rendering inlined per D-13-2.3; field/variant
  name resolution dispatches per place_ty layout walk
  rather than a parallel name-resolution flag).
- *Snapshot stability* (CastKind disambiguation
  by-rule: ambiguous-only annotation per §7.3.1
  rather than always-on noise).

**Prior-art evidence.**
- `prior-art/rustc.md:131-149` (D-13-6): primary-source
  citations at `rustc_middle/src/mir/pretty.rs:1293-1316
  (pre/post-wrap projection chain)`,
  `:1155-1180 (per-Aggregate variant arms)`,
  `:923-1025 (per-TerminatorKind arms / fmt_head +
  fmt_successor_labels)`, `:805 (per-Statement arms)`.
- `prior-art/rustc_public.md:91-118` (D-13-6):
  primary-source citations at
  `rustc_public/src/mir/pretty.rs:229
  (pretty_assert_message enumeration)`,
  `:397 (pretty_aggregate)`,
  `:171 (pretty_terminator_head)`,
  `:204 (pretty_successor_labels)`,
  `:326 (pretty_operand)`.
- `prior-art/charon.md:98-123` (D-13-6):
  per-construct rendering arms via FmtCtx + WithCtx
  wrapper in `charon/charon/src/pretty/fmt_with_ctx.rs`.

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §7 (Body-level renderer
  per-arm specifications).

**Sub-decisions.**

#### D-13-6.1 -- render_place [LOCKED]

Pre/post-wrap projection chain preserving operator
precedence (Field's `(X.<name>: <ty>)` wrap applies
AROUND prior result; outer parens preserve
deref-then-field semantics). Field/variant name
resolution via place_ty walk; both `FieldLayout.name`
and `VariantLayout.name` are non-optional `StringId`
per `spec.md` §11 -- missing-name is invalid IR
rejected by validation, not handled by a defensive
render-side fallback.

#### D-13-6.2 -- render_operand [LOCKED]

Bare `<render_const>` for `Constant(id)`; rejects
Charon `const (...)` wrapper.

#### D-13-6.3 -- render_rvalue [LOCKED]

9-variant table + AggregateKind 7-variant sub-table;
CastKind minimal form (G3-Q1 (b)); RawPtr aggregate
two-operand `(<op_addr>, <op_meta>) as *const/mut <pointee_ty>`
form per `spec.md` §21.3.6.

#### D-13-6.4 -- render_statement [LOCKED]

3-variant table; `SetDiscriminant` renders variant
name from `place_ty(p)`'s `LayoutKind::Enum.shape`
(`VariantLayout.name` non-optional per `spec.md` §11).

#### D-13-6.5 -- render_terminator + sub-renderers [LOCKED]

6-variant TerminatorKind table; `render_terminator`
output ends with `]` (no trailing `;`); the `;` is
appended by `render_basic_block` at composition time
(§7.10). `render_call_callee.Direct(id)` =
`<path><args>` via Instance lookup;
`render_assert_kind(&AssertKind, &Body)` direct adoption
of rustc_public's 8-template patterns (BoundsCheck,
Overflow per BinOp, OverflowNeg, DivisionByZero,
RemainderByZero, MisalignedPointerDereference,
NullPointerDereference, InvalidEnumConstruction). The
`&Body` parameter is required because operand
placeholders inside AssertKind variants render via
`render_operand`.

#### D-13-6.6 -- render_basic_block [LOCKED]

`bbN: { stmt; ...; terminator; }` envelope;
4-/8-space indentation; trailing `;` per-statement
(emitted by `render_statement`); trailing `;` after
terminator is appended HERE at composition time
(`render_terminator` itself produces `<head> -> [labels]`
with no `;`).

---

### D-13-7 -- Worked-example specimens for snapshot stability [LOCKED]

**Question.** Stage-13 §2 principle: "Worked-example
specimens are spec content. ≥1 canonical fragment per
public renderer in `spec-snapshot.md` pin the textual
contract for snapshot stability; they are spec-side
content, not test fixtures." Stage 13 must lock the
specimen format, the budget, the location (per the
emergent D-13-9 spec split, location is in
`pretty-printer.md` §9 not `spec.md` §18), and the
mirroring relationship to insta-snapshot tests.

**Options considered.**

H1 (specimen format):
- (a) Paired blocks: input value (constructed via plain
  Rust ctor + brief comment) + expected rendered output.
  Spec-side verifiable; reader can mentally construct
  the input and check the output.
- (b) Source-code paired: actual Rust input source +
  rendered RegseqModel output. Captures bridge translation
  context but blurs renderer boundary.
- (c) Free-form prose with embedded snippets. Loose;
  harder to verify against.

H2 (specimen budget):
- (a) ≥1 per public renderer + ≥1 per closed-enum-variant
  where variant rendering differs **at the per-arm spec
  level** -- i.e., variants whose rendering follows
  different structural templates. Variants distinguished
  only by policy decoration (e.g., `CastKind` arms whose
  rendering follows the same `<op> as <ty> [(CastKind)]`
  template, with the bracketed annotation toggling per
  the §7.3.1 ambiguity rule) get one representative
  specimen per policy bucket, not one per arm. Total
  ~75 specimens.
- (a-narrowed) Collapse Ty-variant families further;
  collapse AggregateKind sub-arms; collapse AssertKind
  8 templates to one representative (~40 specimens).
- (b) Minimal: ≥1 per public renderer, no per-variant
  coverage (~17 specimens).

H3 (specimen location):
- (a) Embedded inline in §18 spec contributions.
- (b) Collected in single appendix at end of §18.
- (c) Per-stage in `stage-13/spec-snapshot.md` only,
  not top-level.

  (Per emergent D-13-9, location is in
  `pretty-printer.md` §9 not `spec.md` §18; the
  inline-vs-appendix question still applies within
  `pretty-printer.md`.)

H4 (insta-snapshot mirroring):
- (a) 1:1 mirroring: every specimen has a matching
  insta snapshot test in `crates/regseq-model/tests/
  pretty.rs`.
- (b) Specimens illustrative, insta tests independent.
- (c) Specimens mirror a subset; broad insta coverage
  handles edge cases.

**Decision.** **H1 (a) + H2 (a) + H3 (a, scoped to
pretty-printer.md per D-13-9) + H4 (a)**:
- Specimens use paired ctor-input + output-text format.
- ~75 specimens covering ≥1 per public renderer +
  ≥1 per closed-enum-variant whose rendering follows
  a structurally distinct template at the per-arm
  spec level. Variants distinguished only by policy
  decoration (e.g., `CastKind` annotated arms whose
  rendering follows the same `<op> as <ty> [(CastKind)]`
  template, with the bracketed annotation toggling per
  the §7.3.1 ambiguity rule) get one representative
  specimen per policy bucket, not one per arm.
- Inline in `pretty-printer.md` §9 (Worked-example
  specimens) per D-13-9.
- 1:1 mirrored to insta snapshots in
  `crates/regseq-model/tests/pretty.rs` at end-of-phase-1
  implementation sweep.

**Reasoning.**
- H1 (a) keeps the renderer contract crisp: input ctor
  paired with output text. Source-translation context
  (where the value comes from in the bridge) is out
  of D-13-7 scope (stages 5-12 territory).
- H2 (a) per-variant coverage at the per-arm-spec
  level is the pure-discipline-ideal; closes per-variant
  rendering ambiguity at the cost of ~500-800 lines of
  spec content. Per the §2 "Worked-example specimens
  are spec content" principle the cost is justified.
  Variants distinguished only by policy decoration
  (`CastKind` annotated arms; `PointerCoercion`
  sub-variants) get one representative specimen per
  policy bucket (one bare `IntToInt` specimen + one
  annotated `ReifyFnPointer` specimen) rather than one
  per arm; per-template specimens for each individual
  ambiguous variant are unnecessary because the
  rendering template is identical (only the decoration
  string changes verbatim from the variant name).
- H3 (a) inline keeps specimens adjacent to rendering
  rules; reader sees specimen + rule together. Per
  D-13-9 spec split, "inline in §18" becomes "inline
  in `pretty-printer.md` §9."
- H4 (a) 1:1 mirroring catches spec drift directly
  through test failure; canonical test = canonical
  specimen.

**Methodology rules invoked.**
- *Worked-example specimens are spec content* (stage-13
  §2 principle).
- *Snapshot stability* (specimens pin the textual
  contract; insta tests enforce it).

**Prior-art evidence.**
- `prior-art/rustc.md:151-159` (D-13-7): rustc has its
  own MIR test fixtures in the testsuite; convention
  well-established via `--emit=mir` reference outputs
  and golden tests; primary-source location at
  `tests/mir-opt/` and similar fixture directories
  outside the pretty-printer source.
- `prior-art/rustc_public.md:120-128` (D-13-7): tests
  in `tests/ui/stable-mir-print/` (fixture directory;
  not in the pretty-printer source file).
- (No primary precedent for spec-embedded specimens at
  the per-arm spec level granularity -- RegseqModel's
  spec-content discipline per the §2 "Worked-example
  specimens are spec content" principle is its own
  pattern.)

**Spec cross-reference.**
- `pretty-printer-snapshot.md` §9 (Worked-example
  specimens).

**Sub-decisions.**

#### D-13-7.1 -- Specimen format = paired blocks [LOCKED]

Format template:
```text
Input: render_<X>(value) where value is:
    <minimal ctor> // or "the result of <bridge translation rule>"

Output:
    <rendered text>
```

#### D-13-7.2 -- Specimen budget ~71 [LOCKED]

Per `pretty-printer-snapshot.md` §9, specimens cover:
- render_ty: ~20 (1 per Ty-variant family)
- render_place: ~1 (long chain)
- render_operand: ~3 (Copy / Move / Constant)
- render_rvalue: ~17 (9 variants + 8 AggregateKind sub-arms)
- render_statement: ~4 (3 StatementKind + 2 Intrinsic sub-arms; minus overlap)
- render_terminator + sub-renderers: ~17 (6 TerminatorKind + 2 CallCallee + 1 SwitchTargets + 8 AssertKind)
- render_basic_block: ~2 (multi-statement + empty-statements)
- render_item: ~7 (per ItemKind + 1 stress with all attribute prefixes)

Total ~71. Phase B may narrow to ~40 if Codex review
or operational constraints surface; the specimen list
is auditable per-renderer.

#### D-13-7.3 -- Specimen location = inline in pretty-printer.md §9 [LOCKED]

Per D-13-9 spec split. (Inline-vs-appendix question
within `pretty-printer.md` resolved as inline in §9,
adjacent to the §7 body-level renderer specs.)

#### D-13-7.4 -- Insta-snapshot 1:1 mirroring [LOCKED]

End-of-phase-1 implementation sweep wires up insta
tests in `crates/regseq-model/tests/pretty.rs`
mirroring specimens 1:1.

---

### D-13-8 -- §18.1 sigil-table review + §18.2 ownership-table consolidation [CLOSED-BY-ABSORPTION]

**Question (original Open phase framing).** §18.1
sigil table in `spec.md` had identified drift (stale
`BodyId -> Body/N` row in code doc-comment at
`crates/regseq-model/src/pretty.rs:22-34` post-stage-11
D-11-5; stale stage-landing comments at lines 35-36).
§18.2 per-method ownership table was retargeted by
stage-11 D-11-K and stage-12 D-12-K to stage-13 ownership
across all rendering ownership rows. D-13-8 was scoped
to confirm or refine the canonical sigil set + consolidate
the ownership-table presentation.

**Disposition.** **Closed by absorption into D-13-9.**
With the emergent D-13-9 reframe (pretty-printer spec
moves to top-level `pretty-printer.md`), the sigil-table
review lands as `pretty-printer.md` §3 (Body envelope,
indentation, sigil conventions) and the ownership-table
consolidation lands as `pretty-printer.md` §8 (Per-method
ownership table). D-13-8 had no substantive content
beyond what D-13-9 absorbed; the table-cleanup work IS
the §3 + §8 sections of `pretty-printer.md`.

**Status flip at merger contract close.** D-13-8
flips to `[CLOSED-BY-ABSORPTION]` in `decisions.md`
with annotation `(absorbed into D-13-9 per Phase A
discussion)`. The drift items surfaced under D-13-8
(stale `BodyId` sigil; stale stage-landing comments;
stale stub message at `pretty.rs:138-142`) remain
implementation-time concerns per the
spec-before-implementation rule -- they are flagged as
F-13-Code-Drift in §5 below for the end-of-phase-1
implementation sweep.

---

### D-13-9 -- Pretty-printer spec location (companion file) [LOCKED]

**Question (mid-cluster surfacing; user-prompted).**
"Could we create a separate `pretty-printer.md` where
we capture pretty-printer spec instead of adding to
`spec.md`?" Stage-13's pretty-printer spec was
projected to grow `spec.md` §18 by ~1000-2000 lines
(rendering rules + ~71 worked-example specimens). The
user surfaced the architectural question: pretty-printing
is consumer-side concern, separable from the IR's
structural definition. The project already half-implements
the side-file pattern (`architecture.md`, `decisions.md`,
`regseq-background.md`, `rustc-references.md` are
top-level companion files for cross-cutting concerns).

**Options considered.**

I1 (scope of separation):
- (a) Full pretty-printer spec moves to `pretty-printer.md`
  (master style, envelope, per-arm rules, sigil table,
  ownership table, specimens). `spec.md` §18 becomes
  one-paragraph pointer.
- (b) Specimens-only separation (rules stay in §18).
  Half-pregnant; rendering contract split across two
  files.
- (c) Specimens + per-arm rules; sigil + ownership
  tables stay in §18. Splits "structural" from
  "textual" but the line is fuzzy.

I2 (cross-references back to spec.md):
- (a) Drop the deferred-renderer rows from `spec.md`
  entirely (§15 / §19.8 / §20.6 / §21.8 / §22.8 /
  §23.9 / §24.9 / §25.x). Each construct's spec section
  ends with "Rendering: see `pretty-printer.md` §<N>."
- (b) Keep the rows pointing to `pretty-printer.md`.
  Adds noise.
- (c) Keep rows + 1-line summaries in `spec.md`;
  full spec in `pretty-printer.md`. Duplicates content.

I3 (methodology rule extension): README §5.9 needs
rule 6; §5.5 step 5 amendment; §4 folder layout
addition.

I4 (`pretty-printer.md` initial structure):
10-section structure proposed.

**Decision.** **I1 (a) + I2 (a) + I3 (methodology
rule extension) + I4 (10-section structure)**:
- Full pretty-printer spec moves to top-level
  `pretty-printer.md`. `spec.md` §18 becomes
  one-paragraph pointer.
- Drop deferred-renderer rows in `spec.md` §15 / §19 /
  §20 / §21 / §22 / §23 / §24 / §25; replace each with
  one-line "Rendering: see `pretty-printer.md` §<N>"
  pointer.
- README §5.9 gains rule 6 (pretty-printer spec lives
  in `pretty-printer.md`); §5.5 step 5 amended to direct
  pretty-printer spec contributions to `pretty-printer.md`;
  §4 folder layout adds `pretty-printer.md` entry.
- `pretty-printer.md` initial 10-section structure:
  (1) Mission and consumer model; (2) Master rendering
  style; (3) Body envelope, indentation, sigil
  conventions (absorbs §18.1 sigil table); (4)
  Span-rendering and debuginfo modes; (5) Item-level
  rendering shape; (6) Type rendering -- render_ty
  per-arm specs; (7) Body-level rendering -- per-arm
  specs for the 9 public renderers + 1 internal helper;
  (8) Per-method ownership table (absorbs §18.2); (9)
  Worked-example specimens; (10) Cross-references to
  `spec.md` (per-construct rendering pointers).

**Reasoning.**
- I1 (a) cleanest separation: pretty-printing is
  consumer-side concern (snapshot tests, diagnostics),
  factors cleanly from IR-structure spec (producer-side).
  Matches existing top-level side-file pattern. Future
  renderer extensions (additional output formats) extend
  `pretty-printer.md` without touching IR-structure
  spec.
- I2 (a) drops obsolete placeholder rows; the deferred-
  renderer rows in `spec.md` were placeholders pending
  stage 13's ownership and have no further role with
  the spec split.
- I3 is the methodology rule extension stage 13 introduces
  for future renderer-related stages (e.g., stage 14's
  `render_const` and `render_instance`) to inherit.
- I4's 10-section structure mirrors §18 organization
  while absorbing §18.1 (sigils) and §18.2 (ownership
  table) as `pretty-printer.md` §3 and §8.

**Methodology rules invoked.**
- *Spec is mutable* (the side-file pattern is consistent
  with project's existing top-level files).
- *Compositional behavior beats special-case flags*
  (the spec split is a structural reorganization, not
  a parallel system; the merger contract at §5.5 step
  5 absorbs the new file via the existing concat +
  in-place-update mechanism).

**Prior-art evidence.**
- Project's existing top-level companion files
  (in-tree, not external prior-art):
  `docs/discussions/regseq-model/architecture.md` (Q1-Q7
  settled architectural decisions),
  `docs/discussions/regseq-model/decisions.md` (D-tag
  registry),
  `docs/discussions/regseq-model/regseq-background.md`
  (DSL background),
  `docs/discussions/regseq-model/rustc-references.md`
  (pinned rustc-src paths). Pattern is established;
  D-13-9 extends it with `pretty-printer.md`.
- (No external prior-art for the consumer-side /
  producer-side spec split; D-13-9 is RegseqModel-
  internal architectural coherence with the existing
  side-file convention.)

**Spec cross-reference.**
- `pretty-printer-snapshot.md` (whole file -- this is
  D-13-9's principal artifact).
- `spec-snapshot.md` §15 / §18 / §19 / §20 / §21 /
  §22 / §23 / §24 / §25 (cross-reference pointers
  per D-13-9.2).
- `../README.md` §4 / §5.5 step 5 / §5.9 rule 6
  (methodology rule extension per D-13-9.3).

**Sub-decisions.**

#### D-13-9.1 -- Scope of separation = full [LOCKED]

Full pretty-printer spec to `pretty-printer.md`.
`spec.md` §18 becomes one-paragraph pointer.

#### D-13-9.2 -- Cross-references back to spec.md = drop deferred rows + one-line pointers [LOCKED]

`spec.md` deferred-renderer rows (§15.X, §19.8, §20.6,
§21.8, §22.8, §23.9, §24.9, §25.x) drop; each
construct's section gains one-line "Rendering: see
`pretty-printer.md` §<N>" pointer.

#### D-13-9.3 -- Methodology rule extension [LOCKED]

`../README.md` §5.9 gains rule 6: "Pretty-printer
spec lives in top-level `pretty-printer.md`, not
`spec.md`. The two files are co-equal merger artifacts;
integrating per-stage `spec-snapshot.md` content into
either is a stage-13-introduced split. The split
rationale: pretty-printing is consumer-side; IR
structure is producer-side; the line keeps each file
focused on a single audience."

`../README.md` §5.5 step 5 amended: "If stage N's
spec contributions are pretty-printer content,
integrate into `pretty-printer.md` instead of
`spec.md` per §5.9 rule 6. The integration rules
(live-not-historical; topic-based; audit-shape
stripped; no forward-looking placeholders; D-tag
inline references avoided) apply identically."

`../README.md` §4 folder layout gains entry:
`pretty-printer.md           -- pretty-printer spec
(per stage-13 D-13-9; merger artifact alongside
spec.md)`.

#### D-13-9.4 -- pretty-printer.md initial 10-section structure [LOCKED]

(1) Mission and consumer model.
(2) Master rendering style (D-13-1).
(3) Body envelope, indentation, sigil conventions
    (D-13-2 + sigil table; absorbs `spec.md` §18.1).
(4) Span-rendering and debuginfo modes (D-13-3).
(5) Item-level rendering shape (D-13-4).
(6) Type rendering -- `render_ty` per-arm specs
    (D-13-5).
(7) Body-level rendering -- per-arm specs for the 9
    public renderers + 1 internal helper (D-13-6).
(8) Per-method ownership table (absorbs `spec.md`
    §18.2; consolidated under D-13-9.6).
(9) Worked-example specimens (D-13-7).
(10) Cross-references to `spec.md` (per-construct
    rendering pointers).

#### D-13-9.5 -- Working surface = stage-13/pretty-printer-snapshot.md [LOCKED]

Stage-13 maintains `stage-13/pretty-printer-snapshot.md`
for the new top-level file's draft (the bulk content)
+ minimal in-place edits to existing `stage-13/spec-snapshot.md`
for the cross-references per D-13-9.2. At stage-13
merger close, `pretty-printer-snapshot.md` integrates
into top-level `pretty-printer.md` (which is created
fresh -- D-13-9 introduces it); `spec-snapshot.md`
edits integrate into top-level `spec.md` per the
existing merger contract.

#### D-13-9.6 -- D-13-8 closes by absorption [LOCKED]

D-13-8 (§18.1 sigil-table review + §18.2 ownership-table
consolidation) closes by absorption into D-13-9. Sigil-
table review lands as `pretty-printer.md` §3; ownership-
table consolidation lands as `pretty-printer.md` §8.
D-13-8 has no substantive content beyond what D-13-9
absorbs. At merger contract close, D-13-8 flips to
`[CLOSED-BY-ABSORPTION]` in `decisions.md` with
annotation `(absorbed into D-13-9 per Phase A
discussion)`.

---

## 2. Phase A discussion threads (substantive)

Two substantive Phase A threads warrant standalone
recording for Phase B audit-shape capture (in addition
to their per-D-tag entries above).

### Building-block-vs-composer architectural division (D-13-4.6)

A user-prompted mid-cluster surfacing during D-13-4
discussion. The user asked "shouldn't we be rendering
Instances instead of Items?" -- a direct architectural
challenge to the D-12-K bundle's `render_item` ownership
assignment.

The challenge surfaced a real concern: per Q2 (eager
mono baking), the codegen consumer is Instance-keyed;
Function bodies / Static initialisers / Const values /
ADT layouts all live on `InstanceKind` post stage-12
D-12-1.4 Option B reframe. So the headline pretty-printer
consumer should naturally be `render_instance`, not
`render_item`.

**Resolution via building-block-vs-composer split.**
Three options were considered:
- (a) Keep `render_item` public per D-12-K.1 (current plan).
- (b) Make `render_item` private/internal to PrettyPrinter.
- (c) Move `render_item` ownership to stage 14 entirely
  (retarget D-12-K -- heavyweight stage-12 reopen).

(a) was selected. The division: **stage 13 owns NEW
rendering BUILDING BLOCKS (`render_ty`, `render_item`,
all body-level renderers); inherited building blocks
(`render_def_path` stage 12 per D-12-6,
`render_def_path_value` + `render_span` stage 1) are
consumed by composers but not stage-13-owned; stage 14
owns rendering COMPOSERS (`render_const` per D-4-K,
`render_instance` composing render_item + per-Instance
content, top-level `render` iterating Instances).**

The user's challenge clarified that:
- The headline consumer IS `render_instance` (stage 14).
- `render_item` is a useful building block AND an
  Item-table summary view ("list all Items in the
  model"); both rationales preserve its public-API
  status per D-12-K.1.
- Stage 13 owns the building blocks because Items are
  stage-12-territory schema; rendering style for the
  wrapper is naturally stage-13 master-style territory
  (D-11-K's "rendering style ONCE for the whole
  surface" rationale extends compositionally to
  Item-level rendering).

Locked as D-13-4.6.

### Pretty-printer spec location reframe (D-13-9 emergent)

A user-prompted mid-cluster surfacing during D-13-7
discussion. The user asked "could we create a separate
`pretty-printer.md` instead of adding to `spec.md`?"

The challenge surfaced the structural concern: stage
13's pretty-printer spec was projected to grow
`spec.md` §18 by ~1000-2000 lines (rendering rules + ~71
worked-example specimens). The project already
half-implements the side-file pattern (`architecture.md`,
`decisions.md`, `regseq-background.md`,
`rustc-references.md` are top-level companion files
for cross-cutting concerns).

**Resolution via spec split.** Three scopes considered:
- (a) Full pretty-printer spec to `pretty-printer.md`
  (selected).
- (b) Specimens-only separation (rules stay in §18) --
  half-pregnant.
- (c) Specimens + per-arm rules; sigil + ownership
  tables stay in §18 -- fuzzy line.

(a) was selected. The separation:
- `pretty-printer.md` carries the full pretty-printer
  spec (10 sections per D-13-9.4).
- `spec.md` §18 becomes one-paragraph pointer.
- Deferred-renderer rows in `spec.md` §15 / §19.8 /
  §20.6 / §21.8 / §22.8 / §23.9 / §24.9 / §25.x drop
  + replace with one-line "Rendering: see
  `pretty-printer.md` §<N>" pointers.
- README §5.9 / §5.5 step 5 / §4 get methodology
  rule extension per D-13-9.3.

Reframe consequences:
- D-13-8 closes by absorption into D-13-9 (sigil-table
  review + ownership-table consolidation land in
  `pretty-printer.md` §3 + §8).
- Stage 13's working surface gains
  `stage-13/pretty-printer-snapshot.md` for the new
  top-level file's draft.
- Stage-13 merger contract integrates two target
  files (`pretty-printer.md` created fresh; `spec.md`
  in-place edited).

Locked as D-13-9 (emergent main D-tag, not a
sub-decision).

---

## 3. Cross-references

- `../README.md` -- top-level project mission, methodology
  rules, architectural decisions Q1-Q7. Stage 13's
  D-13-9.3 introduces methodology rule extension at §5.9
  rule 6 / §5.5 step 5 / §4 folder layout.
- `../decisions.md` -- registry of all per-stage D-tags;
  stage-13 D-13-1 through D-13-7 and D-13-9 enter as
  `[LOCKED]` at merger close; D-13-8 enters as
  `[CLOSED-BY-ABSORPTION]`.
- `../spec.md` -- merged top-level spec; stage-13
  contributions via in-place edits per D-13-9.2 (drop
  deferred-renderer rows + add one-line cross-references).
- `../pretty-printer.md` -- new top-level file introduced
  by D-13-9; stage-13 contributions land here as the
  bulk authoring per D-13-9.5.
- `../stage-3/discussions.md` -- historical D14 entry
  (preserved as deferred-decision context); referenced
  by D-13-1.3 closure-by-absorption.
- `../stage-11/discussions.md` D-11-K -- master cascade
  retargeting body-level rendering ownership to stage 13;
  D-13-1 through D-13-6 inherit per the cascade.
- `../stage-12/discussions.md` D-12-K -- compositional
  Item-level rendering cascade; D-13-4 inherits.
- `prior-art/rustc.md` -- rustc-internal MIR + Ty
  pretty-printer evidence.
- `prior-art/rustc_public.md` -- rustc_public stable IR
  pretty-printer evidence; closest structural analog.
- `prior-art/charon.md` -- LLBC IR pretty-printer
  evidence; AstFormatter precedent.
- `prior-art/mrustc.md` -- counter-precedent;
  dual-rendering split anti-pattern.

---

## 4. Outbound expectations to file at merger contract close

Per `../README.md` §5.5 step 7, the following outbound
expectations are filed at stage-13 merger contract
close:

### To `../stage-14/expectations.md` "From stage 13"

Stage 14 owns rendering composers (`render_const`,
`render_instance`, top-level `render`); stage-13
master style decisions inherit. Outbound bundle:

```text
1. render_const inherits D-13-1 master rendering style
   (Rust-source-style with sigil decorations + PrettyPrinter
   methods returning String). Stage-4 D-4-K already
   deferred render_const to stage 14; the master style
   for it is now locked at stage 13.

2. render_instance inherits D-13-1 master style and
   composes:
   - render_item (D-13-4 wrapper output: attribute block +
     [unsafe ]fn / static [mut ] / const / struct / enum /
     union kind-keyword + DefPath)
   - render_ty over FnSig for Function Instances
     (signature: `(arg_tys) -> ret_ty`)
   - body envelope per D-13-2 (`{ let _N: T; ...; bbN: { ... } }`)
   - body-level renderers per D-13-6 (per-Statement,
     per-Terminator, per-BasicBlock arms)
   - layout / init / value rendering per per-Instance
     content (Function: body; Static: ty + init; Const: value;
     Struct/Enum/Union: layout)

3. render top-level inherits D-13-2 envelope + iterates
   model.instances (Q2 codegen-keyed). Per-Instance dump
   composed via render_instance.

4. Span / debuginfo gating per D-13-3 (PrettyPrintOptions
   with include_spans / include_debug_names) inherits
   uniformly across composers; render_instance and
   render_const honour the same toggles.

5. Worked-example specimens (D-13-7) per render_const +
   render_instance arms: stage 14 adds specimens for
   each render_const arm (per the 11-variant Const enum
   from D-4-1) + render_instance per-InstanceKind arms,
   inline in pretty-printer.md §9 alongside stage-13's
   specimens.

6. D-13-9 spec split inherits: stage-14's pretty-
   printer spec contributions land in pretty-printer.md,
   not spec.md, per the methodology rule extension at
   ../README.md §5.9 rule 6.
```

Predecessor-stage attribution: stage 13 (pretty-printing).

### To `../README.md` (in-place edit at merger close)

Per D-13-9.3, stage-13 merger contract integrates the
methodology rule extension into `../README.md`:
- §5.9 gains rule 6 (pretty-printer spec lives in
  `pretty-printer.md`).
- §5.5 step 5 amended (pretty-printer content directs
  to `pretty-printer.md`).
- §4 folder layout adds `pretty-printer.md` entry.

### Cross-stage reopenings flagged at merger close

Per stage-13 sub-decisions:

- **Stage-3 D14** (renderer-style call deferred to stage
  13). At stage-13 merger close, D14's status flips to
  `[CLOSED-BY-ABSORPTION]` in `decisions.md` with
  annotation `(absorbed into D-13-1 per stage 13)`.

---

## 5. F-13 cross-stage findings (informational)

Findings surfaced during stage-13 Phase A that are NOT
stage-13 D-tags but worth recording for cross-stage
coherence. Action items at merger close.

### F-13-Code-Drift -- pretty.rs scaffold drift (implementation-time)

The in-code `crates/regseq-model/src/pretty.rs` doc
comments + `render_def_path(ItemId)` stub message
reference state that pre-dates stage-9 / stage-11 /
stage-12 reframes:
- Doc-comment sigil table at lines 22-34 includes a
  stale `BodyId -> Body/N` row. No `BodyId` exists
  post stage-11 D-11-5 + stage-12 D-12-1.4 Option B
  reframe (bodies live as `InstanceKind::Function.body:
  Option<Body>`; no separate ID type).
- Lines 35-36 doc comment says `LocalId`/`BlockId`
  sigils land in stages 10/11. D-11-K retargeted both
  to stage 13.
- Lines 138-142 stub message for `render_def_path(ItemId)`
  says "stage 12 fills". D-12-6 locked the body but
  spec-before-implementation rule defers the code edit.

**Stage-13 disposition.** Implementation-time concerns
per the spec-before-implementation rule. Stage 13's
spec contributions resolve the spec-side drift (§3
sigil table in `pretty-printer.md` is canonical
post-Option-B). The code-side cleanup happens at the
end-of-phase-1 implementation sweep alongside
filling the stub bodies.

**Cross-refs:**
- `crates/regseq-model/src/pretty.rs:22-36, 138-142`
  (stale code).
- `pretty-printer-snapshot.md` §3 (canonical
  post-Option-B sigil table).
- `../status.md` "Deferred items (cross-stage)"
  entry to add at stage-13 merger close noting the
  end-of-phase-1 cleanup task.
