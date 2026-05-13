# Stage 14 status

Last updated: 2026-05-13. **D-14-4.12 LOCKED** at
per-stage Phase A (fifth sub-decision in D-14-4
cluster; third and last of the policy-shaped
sub-aspects split out from D-14-4.2). Non-finite
`Float` values render in the rustc-mirrored
underscore-broken form `inf_f<width>` /
`-inf_f<width>` / `NaN_f<width>` across all 4
FloatKind widths (12 entries: 3 classifications x 4
widths). NaN bit-pattern distinction explicitly out
of v1 scope (all NaN bit patterns collapse to single
`NaN_f<width>` form; matches rustc). Negative zero is
finite (D-14-4.2 form `-0f<width>`, no underscore).
Spec edit landed at `pretty-printer-snapshot.md`
§10.1.1 (Float-rendering precision subsection
extended with a "non-finite values" block containing
the 12-entry table + classification rule + 3
load-bearing notes); §10.1.1 policy-listing entry
for D-14-4.12 updated from sketch to LOCKED.
**All three policy-shaped sub-aspects of D-14-4.2 now
closed** (D-14-4.10 sentinel collapsing + D-14-4.11
hex admission + D-14-4.12 float non-finite). The
scalar-variant surface of `render_const` is fully
spec'd. Remaining 7 sub-decisions cover the
type-context-dependent variants (ZST / Aggregate /
EnumValue / Bytes / NullPtr / Reloc) + type-context
threading (D-14-4.9).

Earlier: 2026-05-13. **D-14-4.11 LOCKED** at
per-stage Phase A (fourth sub-decision in D-14-4
cluster). Confirms decimal-only integer rendering for
v1: no hex form admission for `Int` / `Uint` scalar
variants; no `PrettyPrintOptions::integer_hex` toggle
added to the existing 2-field
`PrettyPrintOptions { include_spans,
include_debug_names }` (stage-13 D-13-3). The
future-extension boundary is recorded: a future
revision can add `integer_render_style:
IntegerRenderStyle` to `PrettyPrintOptions` if
phase-1 testing surfaces register-bit-pattern
diagnostic friction (additive shape; would not break
D-14-4.1's locked API signature). Not pre-allocated
per README §5.9 rule 4 (no forward-looking
placeholders). Spec edit landed at
`pretty-printer-snapshot.md` §10.1.1 policy-listing
(D-14-4.11 entry updated from "table above commits to
decimal-only" sketch to LOCKED commitment with
future-revision note). No spec-table or specimen
revisions needed (D-14-4.2 + D-14-4.10 already
implemented the decimal-only convention).

Earlier: 2026-05-13. **D-14-4.10 LOCKED** at
per-stage Phase A (third sub-decision in D-14-4
cluster). Sentinel-collapsing rule for `Int` / `Uint`
matches rustc: 12 signed MIN/MAX entries (`i8::MIN`
through `isize::MAX`) + 6 unsigned MAX entries
(`u8::MAX` through `usize::MAX`); no unsigned MIN
collapsing (matches rustc's `ConstInt` Debug at
`rustc_middle/src/ty/consts/int.rs:60-79` + `:103-113`).
`Isize` / `Usize` sentinel forms are
target-pointer-width-independent for cross-target
snapshot stability. Sentinel forms carry no `_<width>`
suffix (the type-name prefix already disambiguates).
Spec edit landed at `pretty-printer-snapshot.md`
§10.1.1 (new sentinel-collapsing rule + 18-entry
sentinel table inserted before the decimal-rendering
precision subsection); §9 Specimen 46 revised
(`pl011::regs::MyUnion { byte: 255_u8 }` ->
`pl011::regs::MyUnion { byte: u8::MAX }`) per D-14-4.10
applied to the existing D-14-4.2 output.

Earlier: 2026-05-13. **D-14-4.1 + D-14-4.2 LOCKED
at per-stage Phase A.** First two sub-decisions of the
D-14-4 cluster landed:
- **D-14-4.1** (API signature
  `render_const(ConstId, TyId) -> String`) -- pair-form
  mirrors rustc's `pretty_print_const_value_tcx(tcx, ct,
  ty, fmt)`; consumer-context `TyId` required for the
  Bytes / Aggregate / ZST / NullPtr / Reloc arms whose
  rendering shape depends on type, not const.
  Spec-side edits landed at `pretty-printer-snapshot.md`
  §1.3 composer listing, §3.3 sigil convention, §6.7
  GenericArg::Const dispatch, §8 per-method ownership
  table. §7.2 operand-wrapper line explicitly deferred
  to D-14-4.9 (operand-callsite TyId threading is a
  callsite-wiring question).
- **D-14-4.2** (scalar variants Bool / Char / Int /
  Uint / Float) -- rustc-mirrored underscore-decimal
  convention (`42_i32`, `42_u32`, `42_usize`, `'a'`,
  `1.5f32`); width self-determined by variant payload
  tag; consumer `TyId` ignored for scalar arms.
  Decimal-rendering, char-rendering, and finite-float-
  rendering precision rules all pinned. Spec-side edits
  landed at new `pretty-printer-snapshot.md` §10
  Composer renderers (with §10.1 `render_const` intro +
  §10.1.1 scalar variants per-arm table); §10
  Cross-references renumbered to §11; §9 specimen
  revisions to underscore-decimal form (lines 1212-1215
  / 1220 / 1226 / 1234 / 1249 / 1252 / 1278 / 1295 /
  1432-1433 + narrative at line 664). Hex form
  rejected (`0xFFu8` -> `255_u8`); D-14-4.11 confirms
  decimal-only.

10 sub-decisions remain open for Phase A drafting:
D-14-4.3 (ZST), D-14-4.4 (Aggregate), D-14-4.5
(EnumValue), D-14-4.6 (Bytes), D-14-4.7 (NullPtr),
D-14-4.8 (Reloc), D-14-4.9 (type-context threading +
operand-callsite wiring), D-14-4.10 (sentinel
collapsing), D-14-4.11 (hex admission confirmation),
D-14-4.12 (float non-finite).

Previous: 2026-05-13 (**D-14-4 cluster OPENED at
per-stage Phase A.** Entry steps per README §5.1
completed: Step 4 rustc evidence pass landed at
`prior-art/rustc-const.md` (Q1-Q7 covering
API-signature load-bearing finding, per-variant rustc
forms, recursion shape, operand-wrapper layering; 8
new file-line landmarks appended to
`../rustc-references.md` under a new
"Const rendering (MIR pretty-printer)" section);
Step 5 D-tag allocation surfaces 12 candidate
sub-decisions **D-14-4.1 through D-14-4.12**; Step 6
(Charon / mrustc / rustc_public) **deferred** -- the
rustc-evidence pass found rustc_public delegates back
through `tcx.mir_const_pretty()` rather than
reimplementing, so it carries no independent
prior-art weight; Charon / mrustc consultations run
inline during Phase A drafting if D-14-4.4 (Aggregate
dispatch) / D-14-4.6 (Bytes dispatch) surface design
ambiguity not settled by rustc evidence alone; Step 7
status.md + memory.md refined from grounded D-14-4
plan. **Spec-snapshot inventory correction**: the
actual 11 Const variants per `spec-snapshot.md` §9.7
are `Bool / Char / Int / Uint / Float / ZST / Bytes /
Aggregate / EnumValue / NullPtr / Reloc`; the previous
pre-staged inventory listing
`Str / Tuple / Memory / MemoryStatic / Zst` was a
stale paraphrase).

Previous: 2026-05-13 (**D-14-2 cluster LOCKED at
per-stage Phase B.** Codex Phase B review closed at R6
APPROVE with Lockability:yes / Close-out:YES (6 rounds;
14 distinct findings closed: 0 BLOCK + 11 MINOR + 3 NIT;
trail in `review-log.md`). All 9 sub-decisions (D-14-2.1
through D-14-2.9) locked. **Cross-stage friction**:
D-9-10 REOPEN pending per D-14-2.5 (§23.6.2 B-4 + B-6
exception clauses for `IntrinsicKind::Log(_)` callees per
`I-DiagLog-CallArity`); flip recorded at merger close.
Inbound expectations filed: `../stage-15/expectations.md`
(`register_log_module` glue + device-context joining +
Linux `max_str_args_per_call`); `../stage-17/expectations.md`
(call-site atomic-context fact for §26.8 D5)).

Previous: 2026-05-12 (**D-14-2 cluster drafted at
per-stage Phase A; Codex Phase B review pending.** 9
sub-decisions drafted in audit shape; spec contributions
landed in `spec-snapshot.md` covering author surface,
intrinsic shape, `LogArg` carrier, macro contract, bridge
translation contract, format-template grammar, validation
diagnostics, backend lowering record, deferrals. Adopted
the v2 candidate from `regseq-taxonomy-work/.../03-
diagnostic-logging/refined-construct.md` as the source-of-
truth DSL surface).

Earlier: 2026-05-12 (**D-14-1 + D-14-3 cluster LOCKED at
per-stage Phase A.** Codex Phase B review closed at R3
MINOR with Lockability:yes / Close-out:YES; 3 rounds; 8
findings closed: 1 BLOCK + 4 MINOR + 3 NIT; trail in
`review-log.md`. Stage 14 advanced to D-14-2 cluster).

Earlier: 2026-05-12 (**Stage 14 opened.** Snapshots +
expectations.md ready per README §5.1 steps 1-3;
memory.md + status.md drafted from the pre-evidence-pass
plan per README §5.1 step 7).

## Stage state

**Phase 1 -- domain-agnostic foundation. Stage 14 is
the final phase-1 spec stage** (narrowed from the
original monolithic "Stage 14: Domain-specific
extensions" plan per the 2026-05-12 strategy revision;
see `../README.md` §1.3 + §2 + top-level `../status.md`
"Strategy revision (2026-05-12)" note).

### Open steps completed (per README §5.1)

- [x] **Step 1: stage-14 folder created.** Created
  during the previous monolithic-stage-14 plan;
  retained under the same name since the strategy
  revision kept "Stage 14" rather than renaming to
  "14a" (per the 2026-05-12 sign-off).
- [x] **Step 2: snapshots copied.**
  `spec-snapshot.md` + `pretty-printer-snapshot.md`
  copied from top-level on 2026-05-10 (per D-13-9
  spec split: both files are co-equal merger
  artifacts).
- [x] **Step 3: `expectations.md` read as entry
  checklist.** Trimmed 2026-05-12 to keep only
  foundation items; per-domain-family items relocated
  to `../stage-15/expectations.md` /
  `../stage-16/expectations.md` /
  `../stage-17/expectations.md` /
  `../stage-18/expectations.md` per the strategy
  revision.

### Open steps remaining

- [x] **Step 4: read local rustc sources** relevant
  to D-14-1 (tool-namespace attribute machinery +
  intrinsic recognition codepath). Completed
  2026-05-12 via the rustc evidence pass; findings
  recorded in `prior-art/rustc.md` (Q1-Q4); 10-entry
  "Tool-namespace attributes" section appended to
  `../rustc-references.md`.
- [x] **Step 5: allocate concrete D-tags.** D-14-1
  (with 7 sub-decisions D-14-1.1 through D-14-1.7) +
  D-14-3 (with 7 sub-decisions D-14-3.0 through
  D-14-3.6) allocated and LOCKED at Phase A per
  `discussions.md`. Remaining D-tags from the
  pre-staged inventory (D-14-2 / D-14-4 / D-14-5 /
  D-14-6 / D-14-7 / D-14-8 / D-14-9) still open;
  additional D-tags may emerge per their cluster's
  evidence pass.
- [partial] **Step 6: per-stage prior-art research.**
  rustc prior-art at `prior-art/rustc.md` (Q1-Q4
  covering D-14-1.1 through D-14-1.4 + supporting
  evidence for D-14-1.5 enum shape) completed; other
  projects (Charon, mrustc, rustc_public) deferred
  to their respective cluster's evidence pass when
  load-bearing.
- [partial] **Step 7: refine memory.md + status.md
  from grounded knowledge.** This status.md update
  reflects D-14-1 + D-14-3 closure. memory.md still
  describes the pre-evidence-pass plan; will be
  refined as additional clusters close. `MEMORY.md`
  (top-level auto-memory pointer) untouched -- the
  in-repo per-stage memory.md is the durable record.

## Active focus

**D-14-4 cluster DRAFTING at per-stage Phase A
(opened 2026-05-13).** Entry steps per README §5.1
completed (Steps 4 / 5 / 7; Step 6 deferred -- see
"Pre-staged D-tag inventory" below). Active drafting
phase: per-arm `pretty-printer-snapshot.md`
contributions for the 11-variant `Const` enum. 12
sub-decisions **D-14-4.1 through D-14-4.12** allocated.
**D-14-4.1 (API signature)**, **D-14-4.2 (scalar
variants)**, **D-14-4.10 (Int/Uint sentinel
collapsing)**, **D-14-4.11 (integer hex form
admission: decimal-only)**, and **D-14-4.12 (float
non-finite representation)** LOCKED at Phase A
(2026-05-13). **All three policy-shaped sub-aspects of
D-14-4.2 now closed; the scalar-variant surface of
`render_const` is fully spec'd.** 7 sub-decisions
remain open: D-14-4.3 (ZST), D-14-4.4 (Aggregate),
D-14-4.5 (EnumValue), D-14-4.6 (Bytes), D-14-4.7
(NullPtr), D-14-4.8 (Reloc), D-14-4.9 (type-context
threading + operand-callsite wiring). Next
sub-decision per the "Next action" Phase A plan:
**D-14-4.3** (ZST per-arm rendering) -- first of the
single-variant simple shapes covering the
type-context-dependent variants.

D-14-4 cluster contents (12 sub-decisions; full
audit-shape in `discussions.md` D-14-4 entry; rationale
+ prior-art evidence in `prior-art/rustc-const.md`;
locked sub-decisions struck through):

- **D-14-4** (top-level) -- `render_const(ConstId, TyId)
  -> String` composer renderer; per-arm specs across the
  closed 11-variant `Const` enum. Cluster main entry
  picks up `[LOCKED]` once all 12 sub-decisions close.
- ~~**D-14-4.1**~~ -- **API signature** LOCKED 2026-05-13.
  `render_const(ConstId, TyId) -> String` pair form
  mirrors rustc's
  `pretty_print_const_value_tcx(tcx, ct, ty, fmt)` at
  `rustc_middle/src/mir/pretty.rs:1851`. The consumer's
  `TyId` is required for the `Bytes` / `Aggregate` /
  `ZST` / `NullPtr` / `Reloc` arms whose rendering shape
  depends on type, not const. **Load-bearing.**
- ~~**D-14-4.2**~~ -- **Scalar variants per-arm** LOCKED
  2026-05-13 (`Bool` / `Char` / `Int` / `Uint` /
  `Float`). Rustc-mirrored underscore-decimal
  convention: `true` / `false`; `'a'` (char Debug with
  escapes); `42_i32`, `42_u32`, `42_usize`, `42_isize`;
  floats via `<val>f<width>` finite-only -> `1.5f32`,
  `0f64`. Width self-determined by variant tag; consumer
  `TyId` ignored for scalar arms. Three policy-shaped
  sub-aspects split out: sentinel collapsing
  (D-14-4.10), hex form admission (D-14-4.11 confirms
  decimal-only), float non-finite (D-14-4.12). Specimens
  in §9 revised to underscore-decimal form (`0xFFu8` ->
  `255_u8`; `42i32` -> `42_i32`; etc.).
- **D-14-4.3** -- **`ZST` per-arm.** Render rules by
  consumer Ty: `Ty::Tuple(_)` len 0 -> `()`;
  `Ty::Struct/Enum/Union` fieldless or
  `PhantomData<T>` -> `<render_def_path(ItemId)>`;
  `Ty::FnDef(...)` -> `<render_def_path(InstanceId)>`;
  `Ty::Closure { item_id, .. }` non-capturing -> same;
  any other Ty -> bridge-rejection-shaped fallback
  (TBD: structural form vs validation-error path).
- **D-14-4.4** -- **`Aggregate` per-arm dispatch.**
  Dispatch on consumer `Ty.kind()`:
  - `Ty::Array(_, _)` -> `[c1, c2, ...]`
  - `Ty::Tuple(fields)` -> `(c1, c2, ...)` (n>1),
    `(c1,)` (n==1; trailing-comma Rust syntax)
  - `Ty::Struct { layout, .. }`: unit -> `<path>`;
    tuple-form -> `<path>(c1, c2, ...)`; named-field
    -> `<path> { f1: c1, f2: c2, ... }` (delimiters +
    name lookup per `pretty-printer-snapshot.md` §3.4)
  Aggregate over `Ty::Enum` is `EnumValue`-only by
  construction (per stage-4 dispatch); `Aggregate`
  with enum consumer Ty is rejected.
- **D-14-4.5** -- **`EnumValue` per-arm.** Dispatch on
  the variant's field shape via `Ty::Enum.layout.kind`:
  - unit variant -> `<enum_path>::<variant_name>`
  - tuple variant ->
    `<enum_path>::<variant_name>(c1, c2, ...)`
  - named-field variant ->
    `<enum_path>::<variant_name> { f1: c1, f2: c2 }`
  Variant name lookup via `EnumShape.Single.variant`
  or `EnumShape.Multiple.variants[variant_idx]` per
  `pretty-printer-snapshot.md` §3.4 (non-optional
  `VariantLayout.name`).
- **D-14-4.6** -- **`Bytes` per-arm dispatch.** Dispatch
  on consumer `Ty.kind()` (mirroring rustc's special
  cases at `rustc_middle/src/mir/pretty.rs:1874-1898`):
  - All-uninit shortcut: `<uninit>` (rustc-mirrored)
  - `Ty::Ref(_, Ty::Str)` w/ all-Value bytes -> `"hello"`
    (UTF-8 decode; lossy substitution for non-UTF-8 bytes
    per rustc convention)
  - `Ty::Ref(_, Ty::Slice(u8))` w/ all-Value bytes ->
    `b"hello"`
  - `Ty::Array(u8, N)` w/ all-Value bytes -> `*b"hello"`
    (rustc deref form)
  - else / mixed uninit-Value-Reloc -> structural
    fallback (TBD form; recommend
    `bytes![0x00, 0x01, <uninit>, <reloc:path>]`
    or similar -- final form pinned in Phase A spec
    drafting)
- **D-14-4.7** -- **`NullPtr(TyId)` per-arm.**
  `0x0 as <render_ty(TyId)>` (rustc-mirrored typed-cast
  form per `ty/print/pretty.rs:1816-1826`). The TyId
  carried by the variant gives the pointee type; the
  consumer-context TyId is the same in practice.
- **D-14-4.8** -- **`Reloc(InstanceRef)` per-arm.**
  Dispatch on the InstanceRef's resolved Item kind /
  consumer pointer type:
  - Function ref with `Ty::FnDef`/`Ty::FnPtr` consumer
    -> `<render_def_path(InstanceId)>` (no `as` cast --
    function paths name values directly; matches
    rustc's `pretty_print_const_scalar_ptr` FnPtr arm)
  - Static / vtable / synth `ConstMemoryStatic` with
    pointer consumer Ty -> `<render_def_path> as
    <render_ty(TyId)>`
  - InstanceRef -> InstanceId resolution via
    `model.instances` lookup (existing F4-2 mapping;
    no new infra).
- **D-14-4.9** -- **Type-context threading for
  recursive children.** Caller resolves per-field
  types from the parent Ty's layout and passes per-child
  Ty in (mirrors rustc's `try_destructure_mir_constant_for_user_output`
  returning `(child ConstValue, child Ty)` pairs).
  `PrettyPrinter` has `&model` and walks layouts
  internally. Recursion guarantee: const-graph
  acyclicity invariant per `spec-snapshot.md` §9.7
  line 830-836.
- ~~**D-14-4.10**~~ -- **Sentinel collapsing for
  `Int` / `Uint`** LOCKED 2026-05-13. Rustc-mirrored
  18-entry sentinel set: 12 signed MIN/MAX
  (`i8::MIN`/`i8::MAX`/.../`isize::MIN`/`isize::MAX`)
  + 6 unsigned MAX (`u8::MAX`/.../`usize::MAX`); no
  unsigned MIN collapsing. `Isize`/`Usize` are
  target-pointer-width-independent at the rendering
  level (cross-target snapshot stability). Sentinel
  forms carry no `_<width>` suffix.
- ~~**D-14-4.11**~~ -- **Integer hex form admission**
  LOCKED 2026-05-13. Decimal-only for v1; no
  `PrettyPrintOptions::integer_hex` toggle; the
  `0xFFu8` specimen revision already landed at
  D-14-4.2 (`255_u8`) + D-14-4.10 (`u8::MAX`). Future
  revision can add `integer_render_style:
  IntegerRenderStyle` to `PrettyPrintOptions` if
  phase-1 testing surfaces register-bit-pattern
  friction (additive, non-breaking). Not pre-allocated.
- ~~**D-14-4.12**~~ -- **Float infinity / NaN
  representation** LOCKED 2026-05-13. Rustc-mirrored
  underscore-broken form `inf_f<width>` /
  `-inf_f<width>` / `NaN_f<width>` (12 entries across
  F16/F32/F64/F128). NaN bit-pattern distinction
  out-of-v1-scope (all NaN bit patterns collapse to
  `NaN_f<width>`; future revision can add toggle if
  driver-spec needs surface). Negative zero stays
  finite (`-0f<width>` per D-14-4.2; not non-finite).
  ORIGINAL SKETCH BELOW (kept for traceability):
  Rustc's underscore-broken form
  (`inf_f32`, `NaN_f32`, `-inf_f32`) per
  `ty/print/pretty.rs:1787-1804`. **Recommend matching
  rustc** -- the form parses unambiguously and is
  diagnostically useful.

D-14-2 cluster contents (closed; retained for
trajectory context):

- **D-14-2** (top-level) -- diagnostic-logging is the sole
  phase-1 domain extension; 9 sub-decisions cover the
  full surface end-to-end.
- **D-14-2.1** -- severity vocabulary (4-tuple `Debug /
  Info / Warn / Error`) + dash-form intrinsic NAME mapping
  (`log-debug` / `log-info` / `log-warn` / `log-error`);
  per-level intrinsics (not one with level operand). Lands
  the deferral D-14-1.5 left open.
- **D-14-2.2** -- intrinsic function shape:
  `pub fn log_<sev>(fmt: &'static str, args: &[LogArg<'_>])`
  with `#[inline(never)]` + `#[rseq::intrinsic("log-<sev>")]`
  + `unimplemented!()` body; located at
  `regseq::log::intrinsics::log_<sev>` in
  `#[doc(hidden)] pub mod intrinsics`.
- **D-14-2.3** -- `LogArg` argument carrier: closed
  10-variant enum (U8..U64, I8..I64, Bool, Str(&str));
  width-explicit; `From<T>` / `From<&T>` / `From<&mut T>`
  per value variant; usize/isize routed via cfg; rejected-
  types list per refined-construct.md.
- **D-14-2.4** -- macro contract: declarative `macro_rules!`
  (no proc-macro); fixed expansion shape pins the
  bridge's pattern-match target; arguments evaluated at
  the call site unconditionally (no lazy evaluation;
  target-aware codegen-level elision deferred).
- **D-14-2.5** (LOAD-BEARING) -- bridge translation
  contract: Option A (bridge desugars to flat-arg Call;
  `LogArg::from()` + slice peeled at body-construction
  time); `I-DiagLog-CallArity` named invariant; DiagLogPeel
  pass at §25.7.3; transient fingerprint discipline (no
  side-table storage). **REOPENS D-9-10** (§23.6.2 B-4
  + B-6 carve-outs for Log-intrinsic callees).
- **D-14-2.6** -- format template grammar: `&'static str`
  literal; restricted printf-subset; closed leading
  modifier set `{default, :x, :X, :#x, :#X, :08x, :#010x}`
  for v1 with experiments-track-may-extend note; `{{`
  / `}}` escapes; trailing newline forbidden; embedded
  newlines OK; empty template legal.
- **D-14-2.7** -- bridge-side validation diagnostics:
  D1-D4 + D6 + D7 bridge-enforced; D5 (atomic-context)
  deferred to phase-2 stage 17; diagnostic-message
  quality requirement (D4 must report at author argument
  expression, not at `LogArg::from()` wrapper).
- **D-14-2.8** -- deferrals + scope boundaries:
  `register_log_module` to stage 15;
  device-context joining to stage 15; atomic-context
  analysis to stage 17; codegen-level elision to future
  revision; wider modifier coverage to experiments track;
  `&str` null-termination backend-implementation concern;
  explicit out-of-scope items per refined-construct.md
  (no DSL-level gating, no `emit_structured_event`, no
  function-style API, no author-passed device handle).
- **D-14-2.9** -- backend lowering contract: generic-C
  binding (`printf("[<TAG>] " <fmt> "\n", <args>...)`;
  single emitter; single stdout); Linux kernel binding
  (`pr_*` family for phase-2 stage 15; `dev_*` deferred
  pending device-context joining); 4 informational
  targets (Zephyr / FreeBSD / Windows / macOS) recorded;
  Pipeline A uniformity rule.

Revised cluster ordering (post-D-14-4-open):

1. ~~D-14-1 + D-14-3~~ -- **LOCKED 2026-05-12** (3 rounds;
   8 findings: 1B + 4M + 3N).
2. ~~D-14-2~~ -- **LOCKED 2026-05-13** (6 rounds; 14
   findings: 0B + 11M + 3N).
3. **D-14-4** (`render_const`) -- ACTIVE; Phase A
   drafting started 2026-05-13. 12 sub-decisions
   (D-14-4.1 through D-14-4.12) allocated.
   Pretty-printer-snapshot.md edits start here.
4. **D-14-5** (`render_instance`) -- queued; depends on
   D-14-4 + D-14-1's `domain_attrs` for Item-level
   rendering composition.
5. **D-14-6** (top-level `render`) -- queued; depends on
   D-14-5.
6. D-14-7 (`#[naked]` rejection), D-14-8 (`emission_name`
   retarget), D-14-9 (Specimen 70 split) -- independent;
   small surgical fixes; can land alongside composer
   clusters.
7. `core::ffi::c_void` rejection enforcement -- spec edits
   only; no D-tag.
8. Per-stage cross-cluster Codex review on the full
   `spec-snapshot.md` + `pretty-printer-snapshot.md` once
   all clusters close.
9. Hand off to merger (§5.5 contract).

## Locked decisions

- **D-14-1 -- Q7 intrinsic recognition mechanism + storage
  [LOCKED at R3 MINOR / 2026-05-12]**. 7 sub-decisions
  (D-14-1.1 through D-14-1.7) cover recognition codepath
  (`tcx.get_attrs_by_path`), `register_tool` validation
  delegation to rustc, attribute argument parsing chain,
  malformed-payload validation as bridge responsibility,
  `IntrinsicKind` category + sub-discriminant shape with
  `Log(LogLevel)` stage-14 variant, storage as
  `AttrData::Intrinsic(IntrinsicKind)` in
  `domain_attrs.item_attrs`, and two-phase population timing
  (Phase 2 recognition into `HashMap<DefPathHash,
  IntrinsicKind>` working cache + Phase 4 materialization
  into final sibling table). See `discussions.md` for the
  audit trail; `review-log.md` for the R1-R3 trajectory.

- **D-14-3 -- DomainAttr sibling-table architecture [LOCKED
  at R3 MINOR / 2026-05-12]**. 7 sub-decisions (D-14-3.0
  through D-14-3.6) cover the unified `DomainAttrTables`
  struct with discriminated `AttrData` enum, three sibling
  tables (`item_attrs` / `field_attrs` / `span_attrs` --
  `instance_attrs` dropped per stage-12 D-12-1.8 pull-through
  pattern), append-only `AttrData` ordering, Phase-2-encounter
  / Phase-4-preserved `Vec<AttrData>` ordering, validator-
  enforced per-family duplicate invariants
  (I-DomainAttr-Intrinsic-Singleton committed at stage 14),
  the attribute-namespace storage rule (`#[rseq::*]` ->
  DomainAttr; recognized Rust attrs -> typed Item fields per
  D-12-1.3), and per-family target-table mapping (stage 14
  commits `Intrinsic` -> `item_attrs`).

- **D-14-2 -- Diagnostic-logging vocabulary + bridge
  contract [LOCKED at R6 APPROVE / 2026-05-13]**. 9
  sub-decisions (D-14-2.1 through D-14-2.9) covering the
  full diagnostic-logging surface end-to-end. D-14-2.1
  (severity vocabulary 4-tuple `Debug` / `Info` / `Warn`
  / `Error` + dash-form NAME mapping + per-level
  intrinsics). D-14-2.2 (intrinsic function shape:
  `pub fn log_<sev>(fmt: &'static str, args:
  &[LogArg<'_>])` + `#[inline(never)]` +
  `#[rseq::intrinsic("log-<sev>")]` + `unimplemented!()`).
  D-14-2.3 (`LogArg` carrier: closed 10-variant enum +
  `From` impl matrix + catch-all closure rule). D-14-2.4
  (macro contract: declarative `macro_rules!` + canonical
  expansion shape + argument-evaluation contract).
  **D-14-2.5 (load-bearing): bridge translation contract
  Option A** -- bridge desugars to flat-arg Call;
  `I-DiagLog-CallArity` named invariant; DiagLogPeel pass
  in Phase 5 body materialisation (third body-construction-
  time pass alongside Drop-as-Call; §25.7.3 mechanics +
  reference-companion load synthesis + source-Place
  extraction); transient fingerprint discipline.
  **REOPENS D-9-10** (§23.6.2 B-4 + B-6 carve-outs).
  D-14-2.6 (format template grammar: closed leading
  modifier subset + trailing-newline rule + escape
  sequences). D-14-2.7 (bridge-side validation D1-D8 --
  D5 deferred to stage 17; D8 target-aware
  `max_str_args_per_call`; diagnostic-quality
  requirement). D-14-2.8 (deferrals to stages 15 / 17 +
  future revisions; explicit out-of-scope items).
  D-14-2.9 (backend lowering: generic-C `[ERROR] fmt\n`
  binding + Linux `pr_*` family + `&str` null-term
  contract with normative truncation + 8-arg cap;
  informational targets Zephyr / FreeBSD / Windows /
  macOS recorded). Spec contributions land at §3.6.3
  (NAME mapping) + §23.6.2 B-4/B-6 exceptions + §25.7
  three-pass framing + new §25.7.3 + renumbered §25.7.4
  + new §26 (11 subsections). See `discussions.md` D-14-2
  for the audit trail; `review-log.md` for R1-R6
  trajectory.

- **D-14-4.1 -- `render_const(ConstId, TyId) -> String`
  API signature [LOCKED at Phase A / 2026-05-13]**.
  Pair-form mirrors rustc's
  `pretty_print_const_value_tcx(tcx, ct, ty, fmt)` at
  `rustc_middle/src/mir/pretty.rs:1851`. Consumer-context
  `TyId` is required for the Bytes / Aggregate / ZST /
  NullPtr / Reloc arms (type-dispatched). Spec-side edits:
  `pretty-printer-snapshot.md` §1.3 composer listing,
  §3.3 sigil convention, §6.7 GenericArg::Const
  dispatch, §8 per-method ownership table. §7.2
  operand-wrapper line at 661 explicitly deferred to
  D-14-4.9 (operand-callsite TyId threading depends on
  whether `render_operand` takes an extra `expected_ty:
  TyId` parameter or resolves it internally; both
  options reach the same pair-form invariant). See
  `discussions.md` D-14-4.1 for the audit trail.

- **D-14-4.12 -- Float infinity / NaN representation
  [LOCKED at Phase A / 2026-05-13]**. Rustc-mirrored
  underscore-broken form `<repr>_f<width>` where
  `<repr>` is one of `inf` / `-inf` / `NaN`. Full set
  is 12 entries (3 classifications x 4 FloatKind
  widths: F16 / F32 / F64 / F128). The underscore is
  load-bearing for non-finite values only:
  disambiguates `<repr>` from `f<width>` to a human
  reader (`inff32` would be a single identifier);
  finite values do NOT use the underscore (D-14-4.2
  form). NaN bit-pattern distinction (quiet /
  signaling / payload / sign bit) is **out of v1
  scope**: all NaN bit patterns collapse to the single
  `NaN_f<width>` form (matches rustc's MIR
  pretty-printer behaviour). Future revision can add
  a NaN-bit-pattern rendering option to
  `PrettyPrintOptions` parallel to D-14-4.11's
  hex-admission future-extension boundary (not
  pre-allocated). Negative zero is finite (renders as
  `-0f<width>` per D-14-4.2, not as a non-finite
  case). Classification rule: apfloat's `is_finite()`
  + positive / negative / NaN classification. Spec
  edits: `pretty-printer-snapshot.md` §10.1.1
  Float-rendering precision subsection extended with
  the non-finite handling block; §10.1.1
  policy-listing entry updated. **All three
  policy-shaped sub-aspects of D-14-4.2 now closed**;
  scalar-variant surface fully spec'd. See
  `discussions.md` D-14-4.12 for the audit trail;
  prior-art evidence at `prior-art/rustc-const.md` Q2
  Float subsection +
  `rustc_middle/src/ty/print/pretty.rs:1787-1804`.

- **D-14-4.11 -- Integer hex form admission:
  decimal-only for v1 [LOCKED at Phase A /
  2026-05-13]**. Explicitly records the decimal-only
  commitment already implicit in D-14-4.2. No
  `PrettyPrintOptions::integer_hex` toggle in v1;
  the existing 2-field
  `PrettyPrintOptions { include_spans,
  include_debug_names }` (stage-13 D-13-3) is
  unchanged. Future-extension boundary recorded:
  if phase-1 testing surfaces register-bit-pattern
  diagnostic friction, a future revision can add
  `integer_render_style: IntegerRenderStyle` (closed
  3-variant `Decimal` / `Hex` / `Auto`) to
  `PrettyPrintOptions` -- additive, would not break
  D-14-4.1's locked API signature. NOT pre-allocated
  per README §5.9 rule 4 (no forward-looking
  placeholders). Spec impact: small extension to
  `pretty-printer-snapshot.md` §10.1.1
  policy-listing recording the locked commitment +
  future-revision note. See `discussions.md`
  D-14-4.11 for the audit trail.

- **D-14-4.10 -- `Int` / `Uint` sentinel collapsing
  [LOCKED at Phase A / 2026-05-13]**. Rustc-mirrored
  sentinel set: 12 signed MIN/MAX entries
  (`i8::MIN` / `i8::MAX` / ... / `isize::MIN` /
  `isize::MAX`) + 6 unsigned MAX entries
  (`u8::MAX` / ... / `usize::MAX`); no unsigned MIN
  collapsing (matches rustc; `0_u8` decimal is concise
  enough). `Isize` / `Usize` symbolic forms are
  target-pointer-width-independent for cross-target
  snapshot stability (`usize::MAX` on a 32-bit target
  is still `usize::MAX`, not
  `4294967295_usize`). Sentinel forms carry no
  `_<width>` suffix (the type-name prefix already
  disambiguates: `i32::MAX` not `i32::MAX_i32`).
  Sentinel detection performed at the width-decoded
  value layer (after D-14-4.2's
  sign-extend / zero-extend decoding), against the
  declared width's MIN / MAX constants. Spec-side edits:
  `pretty-printer-snapshot.md` §10.1.1 gains the
  sentinel-collapsing rule + 18-entry sentinel table
  before the decimal-rendering precision subsection;
  §9 Specimen 46 revised
  (`pl011::regs::MyUnion { byte: u8::MAX }`) per the
  sentinel form superseding D-14-4.2's `255_u8` form.
  See `discussions.md` D-14-4.10 for the audit trail;
  prior-art evidence at `prior-art/rustc-const.md` Q2
  Int / Uint subsections.

- **D-14-4.2 -- Scalar variants Bool / Char / Int /
  Uint / Float [LOCKED at Phase A / 2026-05-13]**.
  Rustc-mirrored underscore-decimal convention for all
  five scalar arms. `Bool` -> `true` / `false` (no
  suffix); `Char(u32)` -> `'<ch>'` Rust char-literal
  Debug form with standard escapes; `Int(k, v)` ->
  `<dec>_<width_suffix>` decimal sign-extended at the
  variant's width tag (`42_i32`, `-1_i8`, etc.);
  `Uint(k, v)` -> `<dec>_<width_suffix>` decimal
  zero-extended (`42_u32`, `255_u8`, `16_usize`);
  `Float(k, bits)` -> apfloat-equivalent shortest
  round-trippable decimal + width suffix (`1.5f32`,
  `0f64`). Width is self-determined by the variant
  payload tag (`IntKind` / `UintKind` / `FloatKind` per
  §10.0); the consumer `TyId` from D-14-4.1's pair-form
  is ignored for scalar arms. Three policy-shaped
  sub-aspects split out: sentinel collapsing
  (D-14-4.10), hex form admission (D-14-4.11 confirms
  decimal-only), float non-finite representation
  (D-14-4.12). Spec-side edits: new
  `pretty-printer-snapshot.md` §10 Composer renderers
  with §10.1 `render_const` intro + §10.1.1 scalar
  variants per-arm table + decimal / char / finite-float
  precision rules; §10 Cross-references renumbered to
  §11; §11 table gains §9.7 row pointing to §10.1; §9
  specimen revisions to underscore-decimal form (input
  side: lines 1212-1215 / 1220 / 1226 / 1234 / 1249 /
  1278 / 1295 / 1432-1433; output side: line 1252
  `0xFF_u8` -> `255_u8`; narrative at line 664).
  Hex-form rejection of pre-existing `0xFFu8` specimen
  per the D-14-4.11 forward-commitment (decimal-only is
  final-form for v1). See `discussions.md` D-14-4.2 for
  the audit trail.

- **D-14-10 -- Generic-C backend codegen carve-out
  (companion file) [LOCKED at R3 MINOR / 2026-05-13]**.
  7 sub-decisions (D-14-10.1 through D-14-10.7) covering
  scope of separation, Linux content disposition,
  cross-references, methodology rule extension, initial
  9-section structure for `generic-c-backend.md`, working
  surface, and D-14-2.9 status preservation. Lifts ~600
  lines of generic-C backend codegen content out of
  `spec-snapshot.md` (§26.9 + Appendix A + §25.10 + §19.6
  + §20.4 + §22.7.3 + §10 Ty C-side intent column + §17.1
  fat-pointer C struct table + §17.2.3 trait-object `void
  *` prescription + §13.2 canonical erased pointee +
  §14.x literal C signature example) into the new
  `stage-14/generic-c-backend-snapshot.md` per README §5.9
  rule 7 (introduced by this decision). Linux-target rows
  (§26.9 `pr_*`/`dev_*` family + A.8 MMIO + A.9 atomic)
  dropped wholesale; inbound expectations filed at
  `../stage-15/expectations.md`, `../stage-16/expectations.md`,
  `../stage-17/expectations.md` (each: "From stage 14
  (filed 2026-05-13, per D-14-10)" block). D-14-2.9
  stays LOCKED -- D-14-10 is location-only at the
  decision level (D-13-9 precedent). README updated:
  §4 folder layout adds `generic-c-backend.md` + per-stage
  snapshot; §5.5 step 5/6/7 amended; §5.9 preamble
  updated to "Seven rules"; rule 7 added. Phase B Codex
  trajectory: R1 (Pass A backend snapshot) MINOR / R1
  (Pass B spec.md) BLOCK -> R2 (Pass B exhaustive) BLOCK
  -> R3 (Pass B relaxed-threshold per user) MINOR; R3
  findings addressed; locked. Trail in
  `discussions.md` D-14-10 "Codex Phase B trajectory"
  table.

## Pre-staged D-tag inventory

Working titles from `memory.md` + top-level
`../status.md` "Next action":

- ~~**D-14-1**~~ -- LOCKED 2026-05-12.
- ~~**D-14-2**~~ -- LOCKED 2026-05-13.
- ~~**D-14-3**~~ -- LOCKED 2026-05-12.
- **D-14-4** -- `render_const(ConstId, TyId) -> String`
  per-arm specs (composes `render_ty` from stage-13;
  closed 11-variant `Const` enum: `Bool / Char / Int /
  Uint / Float / ZST / Bytes / Aggregate / EnumValue /
  NullPtr / Reloc`). **Phase A drafting OPENED
  2026-05-13.** 12 sub-decisions D-14-4.1 through
  D-14-4.12 allocated (see "Active cluster" below for
  the full enumeration). API-signature D-14-4.1
  load-bearing.
- **D-14-5** -- `render_instance(InstanceId) -> String`
  per-arm specs (`InstanceKind` dispatch; composes
  `render_item` + `render_ty` + body-level renderers
  + `render_const`). Queued.
- **D-14-6** -- Top-level `render(&self) -> String`
  shape (iterates `model.instances`; composes
  `render_instance`). Queued.
- **D-14-7** -- `#[naked]` rejection in v1; remove
  `is_naked` Item field. Reopens D-12-1.3 + D-9-9 +
  D-13-4.3 (external-review Finding #2). Queued.
- **D-14-8** -- Stale "stage 13 (codegen)"
  forward-pointer retarget (external-review Finding
  #3; pure spec hygiene; no D-tag flip). Queued.
- **D-14-9** -- Specimen 70 split (external-review
  Finding #4; stage-13 internal bug fix; no D-tag
  flip on D-13-7). Queued.

Step-6 (Charon / mrustc / rustc_public) prior-art
**deferred** for D-14-4: the rustc-evidence pass found
that rustc_public delegates back through
`tcx.mir_const_pretty()` rather than implementing its
own dispatch (per `prior-art/rustc-const.md` Q1), so it
carries no independent prior-art weight. Charon /
mrustc consultations run inline during Phase A drafting
if D-14-4.4 (Aggregate dispatch) / D-14-4.6 (Bytes
dispatch) surface design ambiguity not settled by the
rustc evidence pass; per-project findings land at
`stage-14/prior-art/{charon,mrustc}-const.md` if/when
consulted.

## Active cluster

**D-14-4 -- `render_const(ConstId, TyId) -> String`
composer renderer.** Phase A drafting opened
2026-05-13. Working artifact:
`pretty-printer-snapshot.md` (composer renderer specs
land in §6 expansion or new dedicated section per
D-13-9 spec split). 12 sub-decisions allocated; full
audit-shape lands in `discussions.md` D-14-4 entry
during drafting. Full per-arm rationale + rustc evidence
in `prior-art/rustc-const.md`. The cluster contents
listing under "Active focus" above enumerates each
sub-decision with its candidate Phase-A direction.

Per README §5.3, parallel sessions are not planned for
stage 14: D-tag clusters are not independent enough to
parallelize (composer renderers are sequential per the
D-14-4 -> D-14-5 -> D-14-6 dependency chain; Findings
are small enough to land alongside main decisions). Solo
work; no `sessions/` subfolder will be created.

## Cross-stage friction

D-14-2 cluster surfaced one **new** cross-stage friction
point in addition to those previously anticipated:

- **D-9-10 Terminator-site invariants registry --
  REOPEN.** D-14-2.5's bridge translation contract
  introduces an exception to §23.6.2 B-4 (Call args
  length == callee signature arity) and B-6 (per-arg
  type match) for callees carrying
  `IntrinsicKind::Log(_)`. The exceptions are bounded
  by the `I-DiagLog-CallArity` invariant. Flip in
  `decisions.md` at merger close with annotation
  "(revised in stage 14 D-14-2.5: B-4 and B-6 carved
  out for `IntrinsicKind::Log(_)` callees per
  I-DiagLog-CallArity invariant)".

Anticipated friction points still on watch for upcoming
clusters:

- **D-9-9 9-row rejection table extension** (anticipated
  per D-14-7 / Finding #2; not yet processed). Adding the
  `#[naked]` Item-translation rejection row alongside
  existing `InlineAsm` Call-translation rejection shifts
  the table from 9 to 10 rows. Status flip in
  `decisions.md` at merger close.

- **D-13-4.3 per-flag rendering table reduction**
  (anticipated per D-14-7 / Finding #2; not yet
  processed). Removing the `#[naked]` row shrinks the
  per-flag rendering table in `pretty-printer.md`. Status
  flip in `decisions.md` at merger close.

D-14-2 cluster closure did NOT surface friction with
D-13-6 (per-arm `render_terminator` body-level spec):
the `Call` arm renders whatever args the Call carries
element-by-element regardless of arity, so the
post-DiagLogPeel flat-arg shape renders naturally. No
D-13-6 REOPEN.

## Codex review trail

Per-cluster trail recorded in `review-log.md`.

**D-14-1 + D-14-3 cluster trajectory (closed 2026-05-12):**

| Round | Verdict | Findings | Lockability |
| ----- | ------- | -------- | ----------- |
| R1 | BLOCK | 1 BLOCK + 1 MINOR + 2 NIT | no |
| R2 | MINOR | 2 MINOR + 1 NIT | yes / Close-out: YES |
| R3 | MINOR | 1 MINOR | yes / Close-out: YES |

Total: 3 rounds; 8 distinct findings closed (1B + 4M + 3N).
Tighter than every prior stage's Phase B trajectory:
stage-13 4 / 39; stage-12 7 / 31; stage-11 6 / 26; stage-10 5 / 29.
The rustc evidence pass (Step 4) crystallized most of D-14-1's
sub-decisions before Phase B started; the user's design call on
the unified `DomainAttrTables` architecture removed
per-family-vs-unified architectural ambiguity; stage-12 D-12-1.8
pull-through provided a clean template for `instance_attrs`
removal. Full audit at `review-log.md`.

**D-14-2 cluster LOCKED at R6 APPROVE (2026-05-13).** 6
rounds; 14 distinct findings closed (0 BLOCK + 11 MINOR
+ 3 NIT). Trajectory: R1 surfaced 6 findings (1 BLOCK
would have been D-14-2.5's architectural choice, but
pre-Phase-B user sign-off on Option A pre-empted; result
was MINOR-only); R2-R5 progressively tightened the
contract; R6 verified end-to-end consistency and closed
out. Compared to D-14-1+D-14-3 (3 rounds / 8 findings),
D-14-2 ran longer because of larger surface (9
sub-decisions covering full author surface + intrinsic
shape + closed `LogArg` carrier + macro contract +
bridge translation + format-template grammar +
bridge-side validation + per-target lowering record +
deferrals). 0 BLOCK throughout: the load-bearing
architectural choice was pre-validated; MINOR-only
trajectory reflects contract-completion refinement
rather than architectural reframing.

**Anticipated upcoming clusters:** D-14-4 composer
renderer (Phase A drafting active; Phase B Codex review
expected ~3-5 rounds given the 12-sub-decision surface
-- comparable to D-14-2's 6 rounds with similar
multi-sub-decision shape, but smaller scope per
sub-decision since per-arm specs are mostly local);
D-14-5 / D-14-6 composer renderers (sequential, ~2-4
rounds each); D-14-7/8/9 (small surgical fixes, ~1-2
rounds each); per-stage post-cluster cross-validation
review against full spec-snapshot.md +
pretty-printer-snapshot.md (~2-4 rounds at stage
close). Post-merge tier-3 review against top-level
spec.md + pretty-printer.md per D-13-9 / §5.5 step 6
amendment (~2-5 rounds at merger close).

## Next action

**Continue D-14-4 Phase A drafting.** Two sub-decisions
LOCKED (D-14-4.1 API signature + D-14-4.2 scalar
variants). The next concrete drafting step is the
policy-shaped sub-aspects split out from D-14-4.2:
**D-14-4.10 (sentinel collapsing)**, then D-14-4.11
(hex admission confirmation), then D-14-4.12 (float
non-finite representation). These all build on
D-14-4.2's underscore-decimal foundation.

**Phase A drafting plan (revised):**

1. ~~Open D-14-4 entry in `discussions.md`~~ -- DONE
   2026-05-13.
2. ~~D-14-4.1 (API signature)~~ -- LOCKED 2026-05-13.
3. ~~D-14-4.2 (scalar variants)~~ -- LOCKED 2026-05-13.
4. ~~D-14-4.10 (sentinel collapsing)~~ -- LOCKED
   2026-05-13. ~~D-14-4.11 (hex admission
   confirmation)~~ -- LOCKED 2026-05-13.
   ~~D-14-4.12 (float non-finite)~~ -- LOCKED
   2026-05-13. **All three policy-shaped
   sub-aspects of D-14-4.2 now closed; scalar-variant
   surface of `render_const` fully spec'd.**
5. **D-14-4.3 (ZST), D-14-4.7 (NullPtr),
   D-14-4.8 (Reloc)** -- single-variant simple shapes.
6. **D-14-4.4 (Aggregate) + D-14-4.5 (EnumValue) +
   D-14-4.6 (Bytes)** -- the multi-arm dispatch
   sub-decisions. Charon / mrustc inline consultation
   triggered here if dispatch shape ambiguity surfaces.
7. **D-14-4.9 (type-context threading +
   operand-callsite wiring)** -- recursion-shape
   sub-decision binding D-14-4.4 / D-14-4.5 / D-14-4.6,
   plus the §7.2 line 661 operand-callsite wiring
   deferred from D-14-4.1.
8. **Add ~5-10 specimens** to `pretty-printer-snapshot.md`
   §9 per D-13-7 conventions, covering each variant +
   each dispatch arm (estimated specimen counts:
   Bool/Char/Int/Uint/Float covered by single
   representatives; ZST x 3 forms; Aggregate x 4 forms;
   EnumValue x 3 forms; Bytes x 5 forms incl uninit /
   structural fallback; NullPtr x 1; Reloc x 2).
9. **Phase B Codex review** on `pretty-printer-snapshot.md`
   D-14-4 contributions per `memory.md` review-cadence
   note. Iterate to APPROVE.

**Pretty-printer-snapshot.md edits.** Spec content
lands in the new §10 Composer renderers section
(created at D-14-4.2 lock); §10.1 hosts
`render_const`-specific per-arm specs with one
subsection per sub-decision. §10 Cross-references
renumbered to §11.

**Cluster sequence after D-14-4 lock:**
- D-14-5 (`render_instance`) -- depends on D-14-4 +
  D-14-1's `domain_attrs` for Item-level rendering
  composition.
- D-14-6 (top-level `render`) -- depends on D-14-5.
- D-14-7 / D-14-8 / D-14-9 (small surgical fixes;
  independent; can land alongside composer clusters).
- `core::ffi::c_void` rejection enforcement (spec
  edits only).
- Per-stage cross-cluster Codex review on the full
  `spec-snapshot.md` + `pretty-printer-snapshot.md`.
- Hand off to merger (§5.5 contract; flips D-9-10 to
  [REOPENED] in `decisions.md`; integrates §26 +
  §25.7.3/§25.7.4 + §23.6.2 changes into top-level
  `spec.md`; integrates D-14-2 audit trail + D-14-4
  composer-renderer specs into top-level
  `pretty-printer.md`).

## How to update this file

- Update `Last updated` at the top when changes land.
- Mark Open steps `[x]` as they complete.
- Move D-tags from the pre-staged inventory to a
  "Locked decisions" entry when they lock.
- Add entries to "Active cluster" when a cluster's
  work starts.
- Record cross-stage friction as it surfaces.
- Record Codex review rounds in `review-log.md`
  (not here); summarize counts under "Codex review
  trail" above.
- This file mirrors the project-level `../status.md`
  shape but scoped to stage 14 only. Cross-stage
  signals go in `../status.md`, not here.
