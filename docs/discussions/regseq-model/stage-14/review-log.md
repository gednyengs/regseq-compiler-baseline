# Stage 14 -- Review Log

Per-stage adversarial review trail per README §5.1 Work
step 3 + §5.6 tier-1 (collapsed with tier-2 since stage 14
is non-parallelized solo work). Records each round's
verdict, finding counts, and disposition. Round-by-round
detail uses the standard `R<N>` numbering inside each
cluster's subsection.

Codex review uses `scripts/python/codex_runner.py` with
`--effort high --timeout 1500-1800 --message-only`. Prompt
template per project conventions in `../README.md` §6
Operational pointers.

---

## Cluster: D-14-1 + D-14-3 (Q7 intrinsic recognition + DomainAttr architecture)

**Verdict: LOCKED at R3 MINOR with Lockability:yes /
Close-out:YES (2026-05-12).** 3 rounds total; 8 distinct
findings closed (1 BLOCK + 4 MINOR + 3 NIT).

Coverage: spec-snapshot.md (§3 RegseqModel struct field;
§3.3 step 10 validation; §3.4 Phase-4 side-table list;
§3.5.2 14th pull-through helper; §3.6 new "Domain attribute
tables" subsection; §8.0 in-place forward-pointer correction
per Finding #1) + discussions.md (D-14-1 with 7
sub-decisions; D-14-3 with 7 sub-decisions; cross-cutting
recap) + prior-art/rustc.md (Q1-Q4 supporting evidence
from the rustc evidence pass).

### R1 -- 2026-05-12

**Verdict: BLOCK / Lockability: no / Close-out: NO.**

4 findings (1 BLOCK + 1 MINOR + 2 NIT):

1. **[BLOCK]** Final `ItemId`-keyed table population scheduled before
   final `ItemId`s exist. §3.6.4 pushed to
   `model.domain_attrs.item_attrs[id]` during Phase 2, but D-12-5
   locks post-sort `ItemId` assignment in Phase 4. The push site
   either used a non-final `ItemId` or silently required an
   unstated remap. **Resolution:** split recognition from
   materialization. Phase 2 recognizes into an
   `intrinsic_cache: HashMap<DefPathHash, IntrinsicKind>` working
   cache; Phase 4 walks the cache and materializes
   `domain_attrs.item_attrs[id]` alongside `item_def_paths` /
   `item_synthetic_kinds` population. Pattern mirrors §3.4's
   existing `vtables` / `ConstMemoryStatic` / closure-captures
   protocol.

2. **[MINOR]** `domain_attrs` validation omitted top-level
   table/key well-formedness checks. **Resolution:** §3.3 step 10
   + §3.6.5 gained explicit checks: `item_attrs.len() ==
   items.len()`; `FieldPath` keys resolve (parent ADT / variant /
   field in-bounds); `Span` keys satisfy §5.3.

3. **[NIT]** D-14-3.4 audit said "Phase C" but spec correctly
   placed validation in Phase B. **Resolution:** "Phase C" ->
   "Phase B step 10" in D-14-3.4 spec-impact line.

4. **[NIT]** Stale `:1754-1758` line-number locators for the §8
   paragraph after §3.6 insertion shifted lines. **Resolution:**
   replaced with `§8.0` section-level references throughout
   discussions.md + §3.6.8.

Codex prompt: `/tmp/codex_d141_d143_r1.txt` (input via
heredoc).
Codex lockability rationale (excerpt): "The rustc recognition
mechanism itself is well-supported by `prior-art/rustc.md`...
The blocker is not prior-art; it is the construction-phase
mismatch between D-14-3's final `ItemId`-keyed storage and
D-12-5's post-sort ItemId assignment."

### R2 -- 2026-05-12

**Verdict: MINOR / Lockability: yes / Close-out: YES.**

3 findings (2 MINOR + 1 NIT) -- all new, not re-raised from R1:

1. **[MINOR]** `Span` key validation cited §5.3 alone but the
   actual sentinel-aware span-bounds rule lives in §3.3 Phase B.
   **Resolution:** §3.3 step 10 + §3.6.5 now cite §5.3 for
   structural definition AND explicitly inherit the existing
   Phase-B "Source-file span bounds (sentinel-aware)" rule
   (`content: Some(_)` checks `lo <= hi <= content.len()` +
   UTF-8 boundaries; `content: None` accepts as-is with
   `Span::slice` -> None resolution per §5.4).

2. **[MINOR]** D-14-3.3 + §3.6.2 ordering prose still said
   "Phase 2 walk order" despite R1's Phase-2-recognition /
   Phase-4-materialization split. **Resolution:** D-14-3.3 audit
   + §3.6.2 prose rewritten to describe the order as established
   in Phase 2 (per-family cache encounter order) and materialized
   verbatim into the destination `Vec` in Phase 4.

3. **[NIT]** Stale `§8 :1734-1758` locator remaining at
   D-14-3 prior-art evidence. **Resolution:** replaced with
   section-level `§8.0` reference matching the rest of R1/R2
   cleanup.

Codex prompt: `/tmp/codex_d141_d143_r2.txt`.
Codex lockability rationale (excerpt): "The R1 BLOCK is
structurally fixed... The remaining issues are
coherence/locator cleanup, not architecture blockers."

### R3 -- 2026-05-12

**Verdict: MINOR / Lockability: yes / Close-out: YES.**

1 finding (1 MINOR) -- new, not re-raised from R1 or R2:

1. **[MINOR]** §8.0 forward-pointer paragraph still named
   `InstanceId` as a DomainAttr key kind, conflicting with
   D-14-3.1's three-table architecture (`instance_attrs` was
   explicitly dropped). The previous R1/R2 cleanup updated §8.0's
   `tcx.intrinsic_raw` correction but didn't update the adjacent
   sibling-table key-kind enumeration. **Resolution:** §8.0
   paragraph updated to "keyed by `ItemId` / `FieldPath` /
   `Span` (three sibling tables in `DomainAttrTables`...)";
   added cross-reference to §3.6 + D-14-3.1 rationale; framing
   shifted from "stage-14 territory" (forward-pointer) to
   "stage-14 territory (now locked at §3.6)" reflecting actual
   landed state.

Codex prompt: `/tmp/codex_d141_d143_r3.txt`.
Cluster locked at R3.

### Trajectory summary

- 3 rounds total at per-stage Phase B level.
- 8 distinct findings closed: 1 BLOCK (R1) + 4 MINOR (R1.2,
  R2.1, R2.2, R3.1) + 3 NIT (R1.3, R1.4, R2.3).
- The BLOCK was the load-bearing finding (Phase 2 vs Phase 4
  ItemId mismatch); the remaining findings were
  coherence/locator cleanup.
- Compared to prior-stage trajectories:
  | Stage | Phase B rounds | Total findings |
  | ----- | -------------- | -------------- |
  | Stage 7 | 5 | 29 (5B + 24M + 0N) |
  | Stage 8 | 4 | 20 (3B + 16M + 1N) |
  | Stage 9 | 6 | 51 (15B + 30M + 6N) |
  | Stage 10 | 5 | 29 (4B + 16M + 9N) |
  | Stage 11 | 6 | 26 (9B + 17M + 0N) |
  | Stage 12 | 7 | 31 (20B + 11M + 0N) |
  | Stage 13 | 4 | 39 (8B + 28M + 3N) |
  | **Stage 14 cluster #1 (D-14-1 + D-14-3)** | **3** | **8 (1B + 4M + 3N)** |
- The cluster's tight per-stage trajectory reflects: (a) the
  rustc evidence pass (Step 4) crystallized most of D-14-1's
  sub-decisions before Phase B started, (b) the user's design
  call on the unified `DomainAttrTables` architecture removed
  the per-family-vs-unified architectural ambiguity, (c)
  cross-stage pull-through pattern (D-12-1.8) provided a clean
  template for `instance_attrs` removal.

---

## Cluster: D-14-2 (diagnostic-logging vocabulary + bridge contract)

**Verdict: in-progress.** Phase A drafting complete
2026-05-12 covering 9 sub-decisions (D-14-2.1 through
D-14-2.9). Phase B review opened 2026-05-13.

Coverage: stage-14/spec-snapshot.md (§3.6.3 dash-form NAME
mapping; §23.6.2 B-4 + B-6 exception clauses citing
`I-DiagLog-CallArity` invariant; §25.7 three-pass framing;
new §25.7.3 DiagLogPeel pass; renumbered §25.7.4 Pass
scheduling; new §26 11-subsection diagnostic-logging
section) + stage-14/discussions.md (D-14-2 + 9
sub-decisions in audit shape) +
stage-14/status.md (active focus + cross-stage friction)
+ stage-15/expectations.md (inbound `register_log_module`
+ device-context joining) + stage-17/expectations.md
(inbound atomic-context fact for §26.8 D5). Source-of-
truth DSL surface adopted from
`regseq-taxonomy-work/research/construct-discovery-v3/
unified-taxonomy/50-discussions/03-diagnostic-logging/
refined-construct.md` (v2 candidate).

### R1 -- 2026-05-13

**Verdict: MINOR / Lockability: yes / Close-out: YES.**

6 findings (0 BLOCK + 4 MINOR + 2 NIT) -- all addressed
in this round; resolutions land before R2.

1. **[MINOR]** B-4 / B-6 exception clauses keyed on
   `model.domain_attrs.item_attrs[callee_item_id]` (an
   unstated `callee_item_id` derivation), while the
   joint validation walk has a `Direct(InstanceId)`
   callee from the Terminator and `model.instance_intrinsic(id)`
   (§3.5.2) is the pull-through helper for this exact
   lookup. **Resolution:** Reworded both exception clauses
   to trigger when `callee = CallCallee::Direct(id)` and
   `model.instance_intrinsic(id) ==
   Some(IntrinsicKind::Log(_))`. Explicit note added that
   `Indirect(Operand)` callees cannot satisfy the
   log-intrinsic predicate (IntrinsicKind is keyed by
   Instance identity, indirect callees pin no
   InstanceId).

2. **[MINOR]** D6 literal-ness check underspecified
   post-Regseq-const-lowering. §9.7 lowers `&str`
   literals to `Const::Aggregate([Reloc(<static>),
   Uint(Usize, len)])` which doesn't distinguish a
   direct literal from any other `&'static str`
   Constant value. The B-6/D6 wording read like a
   post-RegseqModel check, but the check must run on
   the rustc MIR shape pre-lowering. **Resolution:**
   §25.7.3 step 1 expanded to state literal-ness is
   validated against rustc MIR Const::Slice shape
   before Regseq const lowering; §26.8 D6 row reworded
   to clarify pre-lowering timing; §23.6.2 B-6 exception
   spelled the type as `Ty::Ref(Ty::Str,
   Provenance::SharedBorrow)` and split structural
   shape mismatches (bridge-internal panic) from
   author-facing D6 (literal-ness violation).

3. **[MINOR]** `LogArg` rejection list at §26.4 omitted
   plausible passable Rust shapes (`Ty::FnDef`,
   `Ty::FnPtr`, `Ty::Closure`, tuple / array / slice
   values, interior-mutability wrappers like
   `Cell<T>` / `AtomicU*` / `MaybeUninit<T>`, opaque
   `impl Trait`). The "closed by `From` impl set"
   claim was structurally correct but the named-list
   gaps undermined readability. **Resolution:** §26.4
   added the specific Ty-variant rejections AND added
   an explicit **catch-all closure rule** stating the
   `From` impl matrix is the authoritative closure;
   the named list is documentation. D-14-2.3 audit
   shape in discussions.md mirrored.

4. **[MINOR]** Generic-C `&str -> %s` lowering needed
   a binding null-termination contract -- §26.9 said
   "backend materialises null termination as needed"
   while §26.10 said `&str` null-termination is "not
   a spec axis." For Generic-C as a binding
   implementation target, this is contradictory.
   **Resolution:** §26.9 added a binding `&str`
   null-termination contract for Generic-C: static
   literals emit directly with implicit `\0`;
   non-static `&str` operands use a temporary fixed-size
   stack buffer (size cap pinned at phase-1 sweep);
   binding rule. §26.10 deferral retargeted from
   "spec axis" to "inter-target comparison /
   common-helper extraction" -- each binding backend
   pins its own materialisation, Generic-C is pinned
   here.

5. **[NIT]** §26.9 heading said "(informational)"
   while the section's content had binding Generic-C
   + Linux rows. **Resolution:** Heading renamed to
   "Backend lowering contract" (no parenthetical).
   Opening paragraph reframed to say Generic-C and
   Linux are binding for their respective sweeps;
   the four remaining per-target rows (Zephyr /
   FreeBSD / Windows / macOS) are informational and
   grouped at the section's end.

6. **[NIT]** Pipeline-B warning at §26.9 phrased as
   if backend-reachable, but after DiagLogPeel
   `LogArg` and the carrier slice are gone from
   RegseqModel (§25.7.3), so pipeline-B is
   structurally unreachable unless a backend
   reconstructs the pre-DiagLogPeel carrier.
   **Resolution:** Warning rephrased: "backends MUST
   NOT reconstruct a `LogArg` slice and MUST NOT lower
   any pre-DiagLogPeel carrier shape. The zero-
   runtime-cost-on-emit guarantee rests on this rule."

Cross-stage verifications independently confirmed by
Codex during R1:
- D-9-10 REOPEN scope precise: B-4 + B-6 are the
  ONLY §23.6.2 invariants needing exception clauses.
  B-3 / B-5 / B-7 / B-8 / B-9 hold for post-DiagLogPeel
  Calls unchanged (Log callee remains direct, unit
  return, resolved InstanceId, non-virtual flavor,
  non-divergent target).
- D-13-6 NOT-REOPENED claim verified: stage-13's
  `render_terminator_kind` renders Call args
  element-by-element without signature-arity premise.
  Post-DiagLogPeel flat-arg shape renders naturally;
  no spec change required.

Codex prompt: `/tmp/codex_d142_r1.txt` (input via
heredoc).
Codex output: `/tmp/codex_d142_r1_output.txt`.
Codex lockability rationale (excerpt): "No BLOCK issue
surfaced. Option A's only necessary cross-stage
exceptions are B-4 and B-6; B-3/B-5/B-7/B-8/B-9
continue to hold because the log callee remains a
direct function instance with unit return, resolved
`InstanceId`, non-virtual flavor, and non-divergent
target shape. The stage-13 renderer claim is
verified: `render_terminator_kind` renders `Call`
args element-by-element with no signature-arity
premise, so D-13-6 does not need reopening."

Closing-out logic: even with Codex's Close-out: YES,
running R2 to verify the 6 R1 resolutions don't
introduce new findings (prior D-14-1+D-14-3 trajectory
showed R3 caught a finding R2 missed; safer to verify
than lock at R1).

### R2 -- 2026-05-13

**Verdict: MINOR / Lockability: yes / Close-out: NO.**

**R1 resolutions: 6/6 converged** -- no R1 findings
re-raised. Codex independently verified the D6
pre-lowering framing against §9.7's `Const::Aggregate(
[Reloc, Uint])` shape (the `&str` example pins
`Uint(Usize, 5)`); confirmed the B-4 / B-6 helper-based
predicate (R1.1) is coherent with the joint validation
walk; confirmed the §26.9 binding-vs-informational
split (R1.5) reads cleanly.

2 fresh MINOR findings -- both addressed in this
round; resolutions land before R3.

1. **[MINOR]** One-level reference `LogArg` inputs
   admitted by §26.4's `From<&T>` / `From<&mut T>`
   impls (so authors can write `info!("{}", &x)`) but
   not lowered coherently. DiagLogPeel "recovered
   `<expr_i>` directly" gave back a reference-typed
   Operand, but §26.9's specifier table covers only
   value types + `Ty::Ref(Ty::Str, _)` -- no entry for
   `Ty::Ref(<value type>, _)`. The gap: when the author
   passes `&x` where `x: u32`, the IR Call's args[i]
   would be a `&u32` Operand with no specifier
   coverage. **Resolution:** DiagLogPeel-side
   normalisation -- the peel now synthesises a load
   for reference-companion `From` impls before
   emitting the flat Call. §25.7.3 step 2 expanded to
   describe the load-synthesis: allocate a fresh
   value-typed `_tmp_i` via
   `Body::push_synthetic_local`, append
   `Statement::Assign(_tmp_i, Rvalue::Use(
   Operand::Copy(*<expr_i>)))`, substitute `_tmp_i` as
   the flat Call's arg. The IR Call's
   `args[1..args.len()]` carry uniformly value-typed
   Operands. §23.6.2 B-6 exception clause's admissible-
   types list shrunk to value-typed only.
   §26.4 added a "Reference-companion impls are
   macro-side ergonomics only" paragraph documenting
   the discipline. §26.6 `I-DiagLog-CallArity`
   canonical statement updated to spell args[i] as
   `<value_operand_i>` and explicitly name the four
   admissible value-typed `Ty` shapes. §25.7.3
   "Worked example" expanded with a reference-companion
   case demonstrating step-2 load synthesis.
   discussions.md D-14-2.5 audit shape mirrored with a
   "Reference-companion normalisation" paragraph.

2. **[MINOR]** Generic-C non-static `&str` overflow
   behaviour was TBD in a binding §26.9 row -- the
   phase-1 sweep "picks a concrete size cap, with
   truncation policy at overflow" left observable log
   output underspecified. **Resolution:** §26.9
   non-static `&str` rule reframed from a TBD to a
   binding **normative overflow contract**: (a)
   deterministic truncation when source exceeds cap
   (copy `min(source.len(), cap-1)` bytes + trailing
   `\0`); (b) guaranteed NUL termination in all cases;
   (c) no truncation indicator appended (no `"..."`);
   (d) minimum buffer size 256 bytes (portable lower
   bound on un-truncated `&str` length); (e) per-`%s`-
   site buffer (independent buffers when one call has
   multiple `&str` args); (f) numeric cap above the
   256-byte floor is implementation constant. The
   above contract binds observable behaviour fully;
   the cap value above the floor is the only
   implementation-decision residue.

Cross-stage verifications still hold from R1:
- D-9-10 REOPEN scope precise; B-4 + B-6 are the only
  §23.6.2 invariants needing exception clauses.
- D-13-6 NOT-REOPENED; `render_terminator_kind`
  renders args element-by-element.

Codex prompt: `/tmp/codex_d142_r2.txt` (input via
heredoc).
Codex output: `/tmp/codex_d142_r2_output.txt`.
Codex lockability rationale (excerpt): "The R1.1
helper change is coherent... The D6 pre-lowering
framing matches §9.7... The remaining concerns are
contract-completion issues around admitted reference
arguments and a Generic-C implementation boundary,
not architectural blockers to Option A."
Codex R1 resolution status line: "R1 resolutions: 6/6
converged."

Closing-out logic: Codex marked Close-out: NO,
expecting another round. R3 will verify the 2 R2
resolutions (DiagLogPeel reference-companion
normalisation + Generic-C non-static `&str` overflow
contract) close their findings without introducing
new issues, and probe for any further contract-
completion gaps surfaced by the now-tighter R2 state.

### R3 -- 2026-05-13

**Verdict: MINOR / Lockability: no / Close-out: NO.**

**R1 + R2 resolution status:** R1: 6/6 converged.
R2.1: mostly converged with a procedure-precision gap
(re-raised as fresh finding R3.2). R2.2: re-raised on
exact-capacity edge case (R3.1).

3 MINOR + 1 NIT findings -- all addressed in this
round; resolutions land before R4. Lockability flipped
from yes (R2) to no (R3) because two MINOR findings
touch binding-backend behaviour where the spec gap
makes the contract ambiguous; Codex's conservative call.
No architectural reversal.

1. **[MINOR] (R2.2 re-raised)** Exact-capacity `&str`
   edge case not normatively covered. R2.2's truncation
   trigger said `source.len() > cap` but the algorithm
   copies `min(source.len(), cap-1)` bytes -- for
   `source.len() == cap`, the trigger didn't fire and
   the algorithm-vs-trigger boundary was ambiguous.
   **Resolution:** §26.9 non-static `&str` rule
   restated: algorithm runs unconditionally (copy
   `min(source.len(), cap-1)` bytes + write `\0` at
   index `min(source.len(), cap-1)`); truncation
   "occurs whenever `source.len() >= cap`" (at-or-above
   the cap, not strictly above).

2. **[MINOR]** DiagLogPeel reference-companion load-source
   procedure underspecified. R2.1 used informal
   `Place::from_ref(<expr_i>)` phrasing, but `Place` is
   defined structurally as `{ local, projection }` and
   the procedure didn't show how to extract a Place
   from the recovered Operand. **Resolution:** §25.7.3
   step 2 expanded with an explicit **source-Place
   extraction** sub-rule: match the recovered Operand on
   `Operand::Copy(p_ref)` / `Operand::Move(p_ref)` arms
   (taking `p_ref` as the source reference Place); for
   the `Operand::Constant(_)` arm, panic with a
   `bug!`-style diagnostic (references aren't Constants
   in normal post-cleanup MIR). Deref-source construction
   spelled out: `Place { local: p_ref.local, projection:
   [p_ref.projection..., PlaceElem::Deref] }`. The
   `deref_source` type is `T_value` (pointee of
   `Ty::Ref(T_value_id, _)`); well-formed under stage-5
   §19.7 Place validation via stage-5 D-5-1's
   projection-typing rule (stage-5
   spec-snapshot.md:5444 confirmed by Codex during R3
   probing). `Operand::Copy` confirmed correct for all
   admissible reference-companion source types (the
   pointee `T` is one of `u8..u64` / `i8..i64` / `bool`,
   all `Copy`). §25.7.3 worked example for the
   reference-companion case rewritten to show the
   projection-chain extension explicitly, including
   the pre-DiagLogPeel MIR shape (`_t_ref_head = &head`
   materialising the reference as a local before
   `LogArg::from(Move(_t_ref_head))`).

3. **[MINOR]** Generic-C multi-`%s` stack budget
   unbounded. The §26.9 `cap >= 256` floor plus
   per-`%s`-site buffers, multiplied by an unbounded
   number of `%s` args (D1 only enforces template/arg
   parity), made per-call stack use
   implementation-unbounded in the binding phase-1
   backend contract. **Resolution:** Added **D8**
   diagnostic to §26.8 + corresponding §26.9 binding
   rule: "Maximum 8 `&str` arguments per Log Call"
   (counted across args[1..]; `max_str_args_per_call =
   8` for Generic-C). Reframed from the initially-
   proposed "non-static `&str` count" rule to a simpler
   "all `&str` count" rule because the bridge doesn't
   need provenance analysis to compute D8 -- it reads
   the count directly off the fingerprint's `arg_tys`
   field. The bound is strictly tighter than necessary
   (static-literal `&str` args don't allocate buffers
   in practice) but the simpler formulation avoids
   bridge-side provenance analysis. 8 args at the
   256-byte cap floor gives `2KB` worst-case stack use
   per Log Call -- comfortably below realistic stack
   budgets on every binding target while accommodating
   typical driver-logging patterns of 0-3 `&str` args
   per call. Authors needing more compose multiple
   `log!` calls.

4. **[NIT]** §25.8 LocalDecls allocation API consumers
   table omitted DiagLogPeel. Stale documentation after
   R2.1 -- DiagLogPeel's load synthesis allocates fresh
   value-typed locals via `body.push_synthetic_local`,
   making it the third synthesised-locals consumer.
   **Resolution:** Added a new row to the §25.8
   consumers table for DiagLogPeel: one
   `body.push_synthetic_local(T_value_id)` per
   reference-companion log argument; by-value `From<T>`
   and `From<&'a str>` impls allocate no synthetic
   locals.

Cross-stage verifications confirmed during R3:
- Stage-5 §19.7 Place validation + D-5-1 projection-
  typing rule confirmed valid for the bare-ref-local
  deref pattern (stage-5 spec-snapshot.md:5444).
  DiagLogPeel's load synthesis is well-formed under
  stage-5.
- Three Phase-5 body-construction-time passes
  (Drop-as-Call collapse, DiagLogPeel, regular Call
  translation) coexist coherently in §25.7.4 pass
  scheduling diagram: per-rustc-Terminator processing
  with no inter-pass local-allocation race
  (Drop-as-Call handles `Drop` Terminators;
  DiagLogPeel handles `Call` Terminators with
  IntrinsicKind::Log(_) callees; the two are mutually
  exclusive per-rustc-Terminator).

Codex prompt: `/tmp/codex_d142_r3.txt`.
Codex output: `/tmp/codex_d142_r3_output.txt`.
Codex lockability rationale (excerpt): "R2.1's
architectural direction converges: the flat call
invariant now carries only value-typed operands, and
stage-5's `Deref` rule validates a bare ref-local
deref place when the source place is made explicit.
R2.2 mostly converges, but the exact-capacity overflow
edge and unbounded per-call stack allocation are
binding-backend gaps in the phase-1 implementation
surface. These are narrow spec fixes, not architectural
reversals."

Closing-out logic: Codex marked Close-out: NO with
lockability:no. R4 will verify the 4 R3 resolutions
close their findings without re-raising or introducing
new contract-completion gaps. Pattern matches stage-13's
multi-round trajectory at per-stage Phase B; lockable
state expected by R4 or R5.

### R4 -- 2026-05-13

**Verdict: MINOR / Lockability: no / Close-out: NO.**

**Round-by-round resolution status:** R1: 6/6; R2: 2/2;
R3: 4/4 verified (Codex independently confirmed R3.1,
R3.2, R3.4 converge as written; R3.3 directionally
correct but introduced a target-aware diagnostic
without fully updating the validation trigger contract).
R2.2 re-raise from R3.1 confirmed closed. R4 adds 1
fresh MINOR.

1 MINOR finding -- addressed in this round; resolution
lands before R5.

1. **[MINOR]** D8 target-aware trigger underspecified.
   R3.3 introduced D8 as a target-aware diagnostic
   (`max_str_args_per_call` declared per binding target;
   Generic-C: 8) but: (a) §25.7.3 step 5 validation list
   listed only D1-D4 + D6 + D7 -- D8 missing; (b) the
   fingerprint shape `{severity, fmt_template, arg_tys,
   span}` doesn't carry target identity, so the trigger
   context was unclear; (c) Linux is a binding target
   but didn't declare its own `max_str_args_per_call`.
   **Resolution** (4 spec edits):
   - §25.7.3 "Per-position validation" paragraph
     expanded to include D8 alongside D1-D4 + D6 + D7,
     distinguishing the target-independent diagnostics
     (D1-D4, D6, D7) from the target-aware D8. The
     bridge validation pass receives the binding-target
     identity as operational context (via the build's
     target-selection flag or equivalent).
   - §26.8 D8 row reworded to clarify per-(call site,
     target) evaluation: "evaluated once per (call site,
     target) pair: a single build is one target;
     cross-target validation (when a future stage adds
     multi-target build) repeats D8 per target."
     Trigger column now says "post-DiagLogPeel
     fingerprint validation against the build's
     selected target."
   - §26.9 Linux row gained a "Linux `max_str_args_per_call`
     declaration: deferred to stage 15" paragraph. Bound
     couples to Linux's `&str` materialisation strategy
     (`printk` built-in copy vs. stack buffer vs.
     `kasprintf`); stage 15 owns the strategy choice
     and declares the bound alongside.
   - Stage-15/expectations.md "From stage 14" section
     gained a new inbound item requiring stage 15 to
     declare Linux's `max_str_args_per_call` when it
     pins the Linux `&str` materialisation strategy.
     Item enumerates the candidate strategies and notes
     that stack-buffer strategies should match
     Generic-C's 8-arg bound (kernel stack budget
     consideration; `8 * 256 = 2KB` fits 8-16KB
     kernel stacks comfortably).

Codex prompt: `/tmp/codex_d142_r4.txt`.
Codex output: `/tmp/codex_d142_r4_output.txt`.
Codex lockability rationale (excerpt): "R3.1, R3.2,
and R3.4 converge as written... R3.3 is directionally
correct and preserves the simpler 'count all `&str`'
trade-off, but it introduced a target-aware diagnostic
without fully updating the validation trigger and
per-target declaration contract. That is small and
local, but it should be closed before locking Stage 14
because implementation sweep #1 follows immediately."

Closing-out logic: Codex marked Close-out: NO with
lockability:no, but R4 surfaced only 1 finding
(structurally identical to "complete the R3.3
follow-through"). Convergence accelerating; expect
R5 to lock or surface one final cleanup finding.

### R5 -- 2026-05-13

**Verdict: MINOR / Lockability: no / Close-out: NO.**

**Round-by-round resolution status:** R1: 6/6; R2: 2/2;
R3: 4/4; R4: 1/1. All prior-round resolutions remain
converged.

1 MINOR finding -- pure audit-trail consistency
omission; addressed in this round.

1. **[MINOR]** D-14-2.7 audit trail still cited "Seven
   author-facing diagnostics D1-D7" with a table
   stopping at D7, while the canonical spec at
   §25.7.3 + §26.8 now includes D8 (added at R3.3 +
   completed at R4.1). Pure mirror-update issue: the
   spec-side resolution converged at R4 but the audit
   shape in `discussions.md` D-14-2.7 wasn't updated
   to reflect D8.
   **Resolution:** Updated `stage-14/discussions.md`
   D-14-2.7 audit shape:
   - "Decision" paragraph: "Seven" -> "Eight"
     author-facing diagnostics; table extended with
     D8 row; target-independent vs. target-aware split
     marked in the Stage-14 status column.
   - New paragraph below the D5 deferral note
     documenting D8's target-aware framing (Generic-C
     bound = 8; Linux bound deferred to stage 15;
     per-(call site, target) evaluation).
   - "Reasoning" paragraph expanded: enumerates the
     seven bridge-enforced rules + D5 deferral; adds
     a load-bearing-rationale paragraph for D8 (per-
     `%s`-site stack-budget concern; target-aware
     framing; "count all `&str`" simplification
     trade-off).
   - "Spec impact" paragraph references §25.7.3 +
     §26.8 + §26.9 + the stage-15 inbound expectation.
   - New "Inbound expectations filed" subsection lists
     the stage-15 `max_str_args_per_call` inbound.

Codex prompt: `/tmp/codex_d142_r5.txt`.
Codex output: `/tmp/codex_d142_r5_output.txt`.
Codex lockability rationale (excerpt): "The R4
resolution closes the original target-aware trigger
gap in the canonical spec... The stage-15 inbound item
is complete... The remaining issue is local but
contract-visible because the decision audit trail
still records the pre-D8 diagnostic set."

Closing-out logic: R5's sole finding was a pure
audit-trail mirror-update (no spec semantics changed).
R6 should converge to APPROVE / Lockability: yes /
Close-out: YES.

### R6 -- 2026-05-13

**Verdict: APPROVE / Lockability: yes / Close-out: YES.**

**No findings.** Cluster LOCKED at per-stage Phase B.

Codex confirmed all six D8-related sites are internally
consistent end-to-end:
- §25.7.3 + §26.8 both define D8 as target-aware bridge
  validation against the selected target's
  `max_str_args_per_call`.
- §26.9 Generic-C rule counts all `Ty::Ref(Ty::Str, _)`
  operands in args[1..], excludes args[0], uses the same
  8 bound.
- §26.9 Linux paragraph + stage-15 expectation both
  preserve the deferral and the strategy-dependent
  declaration.
- D-14-2.7 audit trail in discussions.md mirrors the
  spec contract (eight diagnostics, target-aware D8,
  Generic-C bound 8, Linux stage-15 inbound,
  per-(call site, target) evaluation).

**Round-by-round final tally:** R1: 6/6, R2: 2/2, R3:
4/4 (with R2.2 re-raise closed at R3.1), R4: 1/1, R5:
1/1, R6: 0 findings.

Codex prompt: `/tmp/codex_d142_r6.txt`.
Codex output: `/tmp/codex_d142_r6_output.txt`.

### Trajectory summary

**Total: 6 rounds; 14 distinct findings closed**
(0 BLOCK + 11 MINOR + 3 NIT).

Round-by-round:
- R1: 4 MINOR + 2 NIT (Lockability: yes / Close-out: YES
  but ran R2 for due diligence per project pattern).
- R2: 2 MINOR (fresh contract-completion gaps surfaced
  post-R1 tightening).
- R3: 3 MINOR (1 R2.2 re-raised; 2 fresh) + 1 NIT.
  Lockability flipped to "no" due to binding-backend
  ambiguities.
- R4: 1 MINOR (D8 target-aware trigger follow-through
  from R3.3).
- R5: 1 MINOR (D-14-2.7 audit-trail mirror to spec
  changes from R3 + R4).
- R6: APPROVE / Lockability: yes / Close-out: YES.

Compared to prior-stage trajectories:

| Stage | Phase B rounds | Total findings |
| ----- | -------------- | -------------- |
| Stage 7 | 5 | 29 (5B + 24M + 0N) |
| Stage 8 | 4 | 20 (3B + 16M + 1N) |
| Stage 9 | 6 | 51 (15B + 30M + 6N) |
| Stage 10 | 5 | 29 (4B + 16M + 9N) |
| Stage 11 | 6 | 26 (9B + 17M + 0N) |
| Stage 12 | 7 | 31 (20B + 11M + 0N) |
| Stage 13 | 4 | 39 (8B + 28M + 3N) |
| Stage 14 cluster #1 (D-14-1 + D-14-3) | 3 | 8 (1B + 4M + 3N) |
| **Stage 14 cluster #2 (D-14-2)** | **6** | **14 (0B + 11M + 3N)** |

The cluster ran longer than D-14-1+D-14-3 because:
- D-14-2 carried a larger surface (9 sub-decisions vs.
  D-14-1+D-14-3's 14 collectively but smaller per
  sub-decision; D-14-2's surface includes full author-
  surface + intrinsic shape + closed `LogArg` carrier +
  macro contract + bridge translation + format-template
  grammar + bridge-side validation + per-target lowering
  record + deferrals).
- 0 BLOCK throughout: the load-bearing architectural
  choice (D-14-2.5 Option A bridge desugaring) was
  pre-validated by the user before Phase B opened
  (resolving the BLOCK risk Codex would otherwise have
  raised at R1). The MINOR-only trajectory reflects the
  surface's contract-completion-style refinement rather
  than architectural reframing.
- Convergence accelerated after R3: R4 + R5 + R6 each
  introduced or closed at most 1 finding. R2.2 was the
  only re-raised finding across the trajectory.

**Cross-stage friction surfaced**: D-9-10 REOPEN
(§23.6.2 B-4 + B-6 exception clauses for
`IntrinsicKind::Log(_)` callees per
`I-DiagLog-CallArity`). Flip in `decisions.md` at merger
close with annotation "(revised in stage 14 D-14-2.5)".
No other REOPEN required. D-13-6 (per-arm
`render_terminator` body-level spec) verified
NOT-REOPENED.

**Inbound expectations filed**:
- `../stage-15/expectations.md`: `register_log_module`
  glue + device-context joining + Linux
  `max_str_args_per_call` declaration (3 items).
- `../stage-17/expectations.md`: call-site
  atomic-context fact for §26.8 D5 (1 item).

---

## Cluster: D-14-10 (Generic-C backend codegen carve-out -- companion file)

Audit-shape decision text and full Codex Phase B trajectory
table also live in `discussions.md` under D-14-10.

### R1 Pass A -- 2026-05-13 (backend snapshot: generic-c-backend-snapshot.md)

**Verdict:** MINOR (1 BLOCK + 2 MINOR + 1 NIT).

**Findings:**

- **R1.PA.1 [BLOCK]** -- §7 (Body-to-C function lowering)
  reads as fragment: promises per-statement / per-terminator
  C-emission subsections, but file proceeds to logging /
  examples without defining them. Fix: narrow §7 to the Body
  envelope only (signature template + locals + BB labels);
  per-terminator emission deferred to phase-1 implementation
  sweep.
- **R1.PA.2 [MINOR]** -- §3 cross-reference is stale:
  spec.md §17.2.1 is "Recursive interior-mutability
  detection", not the trait-object dispatch carve-out. Fix:
  retarget cross-refs to the renumbered §17.2.3 (paired with
  R1.PB §17 numbering finding).
- **R1.PA.3 [MINOR]** -- §8.2 per-arg specifier table uses
  `UintTy` / `IntTy`, but producer-side enum names are
  `UintKind` / `IntKind`. Fix: rename to match producer-side.
- **R1.PA.4 [NIT]** -- §9 worked-example preamble still
  names Linux MMIO / atomic examples (despite the carve-out
  rule that phase-1 generic backend snapshots should not
  carry phase-2 target content). Fix: reframe preamble
  target-neutral.

Codex prompt: `/tmp/codex_d1410_passA.txt` (timeout 540s,
effort high).

### R1 Pass B -- 2026-05-13 (spec.md trim coherence)

**Verdict:** BLOCK (2 BLOCK + 2 MINOR + 1 NIT).

**Findings:**

- **R1.PB.1 [BLOCK]** -- Backend-codegen leakage in 5 cited
  spots: `__attribute__((noreturn))` (3685), `void *` (4541),
  `uint32_t f(uint32_t)` (5379), `memcpy(...)` (8259),
  `pr_*` / `printf` (12206). Fix: lift each to backend
  artifact; leave abstract IR-side claims in spec.md.
- **R1.PB.2 [BLOCK]** -- §17 sized-pointee section retains
  concrete C-qualification rules (`const T*` / `T*` /
  `T* restrict`); only the wide-pointee carve-out was
  trimmed. Fix: reframe as abstract mutability rule + lift
  C-type rendering to backend §3.1.
- **R1.PB.3 [MINOR]** -- §17 cross-references internally
  stale: §17.2.1 heading is "Recursive interior-mutability
  detection", but text + backend refs call the trait-object
  carve-out §17.2.1. Fix: number unnumbered subsections
  (§17.2.2 Wide-pointee, §17.2.3 Trait-object) + retarget
  refs.
- **R1.PB.4 [MINOR]** -- §10 unit/ZST bullet still embeds
  "lowers to C `void` in function return position". Fix:
  reframe IR-side; pointer to backend §2.
- **R1.PB.5 [NIT]** -- Lifted-pointer section anchors
  cross-checked accurate (§10 -> §2, §19.6 -> §4,
  §22.7.3 -> §6, §25.10 -> §7, §26.9 -> §8, Appendix A ->
  §9).

Codex prompt: `/tmp/codex_d1410_passB.txt` (timeout 540s,
effort high).

### R2 Pass B -- 2026-05-13 (exhaustive leak scan)

**Verdict:** BLOCK (~50 leak citations across 15+ topic
areas).

R2 deliberately requested an exhaustive scan to surface
any residual backend-codegen prescriptions beyond the R1.PB
5-spot citation. R2 cited additional categories: Item field
comments (inline_hint, is_naked, opaque_handle_eligible),
§10.3 wide-scalar C compiler rejection, §11 Bool/Char niche
storage lvalue, §13.2 erased-pointee paragraphs (already
partially fixed by R1.PB.1 follow-through), §14 FnSig
admissibility (`void` / `noreturn` / `uint32_t` framing),
§17.2 sized-pointee rule, §22.2.3 NonDivergingIntrinsic
C-emission, §24 panic codegen pattern, §26 outside §26.9
(target emitter names, terminator append, materialisation
strategy).

**User-supplied threshold** narrowed iteration: "We just
want to make sure we lift C generation strategy into a
separate document. Comments in the spec that just clarify
or lightly hint at some C lowering don't have to be
removed." R2 findings classified as KEEP (light hints) vs
LIFT (strategy-level) under this rule -- R2 took the
exhaustive view; R3 applied the threshold.

Codex prompt: `/tmp/codex_d1410_passB_r2.txt` (timeout 540s,
effort high).

### R3 Pass B -- 2026-05-13 (relaxed-threshold check)

**Verdict:** MINOR (6 strategy-level escapes).

**Findings (all addressed in this round):**

- **R3.1 [MINOR]** -- Opaque-handle incomplete-vs-complete
  C declaration behavior (`opaque_handle_eligible` field
  comment + §14.5 prose). Fix: retain eligibility criteria;
  reframe emission form as backend-owned.
- **R3.2 [MINOR]** -- `Const::Aggregate` doc-comment
  prescribes use-site compound-literal C codegen rule. Fix:
  retain IR-side property (no program-image storage);
  point to backend for use-site rendering.
- **R3.3 [MINOR]** -- Bool/Char niche storage-lvalue rule
  names `uint8_t` / `uint32_t` storage. Fix: reframe as
  "unsigned-integer cell of scalar storage width" with
  backend rendering pointer.
- **R3.4 [MINOR]** -- `__attribute__((noreturn)) void`
  appears at three FnSig admissibility sites (5208, 5505,
  5571). Fix: reframe each as IR-side admissibility
  (output MAY be `Ty::Never`; per-target noreturn rendering
  per backend).
- **R3.5 [MINOR]** -- §17.2.3 trait-object cross-ref
  parenthetical embeds `void *` concrete example. Fix:
  drop the example; pointer to backend §3.3 only.
- **R3.6 [MINOR]** -- Diagnostic-logging cap/materialization
  values still embedded in §26.8 D8 narrative + §26.7
  trailing-newline pointer + §26.10 deferrals pointer. Fix:
  abstract D8 bound to "backend-declared per-target";
  retarget §26.9 pointers to `generic-c-backend.md` §8.x.

Codex prompt: `/tmp/codex_d1410_passB_r3.txt` (timeout 360s,
effort medium).

Cluster locked at R3.

### Trajectory summary

**Total: 4 rounds across two parallel passes; ~12 distinct
findings closed at the user's relaxed threshold** (3 BLOCK +
~8 MINOR + 2 NIT; the R2 exhaustive count of ~50 cited
locations was scoped down to the 6 strategy-level escapes
that R3 verified against the user's threshold).

Round-by-round:
- R1 Pass A: 1 BLOCK + 2 MINOR + 1 NIT (backend snapshot
  fragment + cross-refs + naming).
- R1 Pass B: 2 BLOCK + 2 MINOR + 1 NIT (spec.md 5 cited
  leaks + §17 numbering + §10 unit bullet + anchor
  cross-check).
- R2 Pass B: ~50 citations (exhaustive scope; user
  threshold relaxed at R2 close).
- R3 Pass B: 6 MINOR strategy-level escapes (opaque-handle
  + Const::Aggregate + niche storage + 3 noreturn sites +
  void* parenthetical + diag-logging cap embeds); all
  addressed.

Compared to prior cluster trajectories:

| Stage | Phase B rounds | Total findings |
| ----- | -------------- | -------------- |
| Stage 13 cluster #4 (D-13-9, pretty-printer.md carve-out) | 3 | 11 (1B + 7M + 3N -- approximate; per stage-13 trail) |
| Stage 14 cluster #1 (D-14-1 + D-14-3) | 3 | 8 (1B + 4M + 3N) |
| Stage 14 cluster #2 (D-14-2) | 6 | 14 (0B + 11M + 3N) |
| **Stage 14 cluster #4 (D-14-10, generic-c-backend.md carve-out)** | **3 effective (R1 PA + R1 PB; R2 + R3 with relaxed threshold count as a single calibration round)** | **~12 (3B + 8M + 2N) -- at user threshold; R2's exhaustive count of ~50 reduced by threshold calibration** |

The cluster's distinctive features:
- **Parallel passes** (Pass A on backend snapshot; Pass B
  on spec.md): first stage-14 cluster to run two
  Codex-prompt passes in parallel for independent surfaces.
- **External-audit-triggered**: D-14-10 was prompted by an
  external audit pass (not pre-staged in the original
  D-14-1..D-14-9 inventory); the audit identified ~600
  lines of generic-C backend codegen content embedded in
  spec.md.
- **Threshold calibration mid-trajectory**: R2 exhaustive
  scan exposed ~50 cited locations; user's "lift strategy /
  keep light hints" threshold narrowed iteration to the 6
  R3 escapes. R3 ran at effort=medium / timeout=360s
  rather than the prior rounds' high/540s.
- **Concurrent with active D-14-4 cluster**: cluster
  serialisation by file (D-14-10 work confined to
  `spec-snapshot.md`, `generic-c-backend-snapshot.md`,
  `README.md`, `discussions.md` end-append; D-14-4 work
  on `pretty-printer-snapshot.md` + memory.md). No
  shared-file collisions surfaced.

**Cross-stage friction surfaced**: None. D-14-10 is
relocation-only at the decision level per D-14-10.7 --
D-14-2.9 stays LOCKED; no earlier-stage D-tag reopens.

**Inbound expectations filed**:
- `../stage-15/expectations.md`: Linux binding for the
  diagnostic-logging surface has no shadow contract in
  stage-14 spec; phase-2 stage 15 re-derives against
  pinned MIR + kernel-API evidence (1 block).
- `../stage-16/expectations.md`: MMIO worked example
  dropped from stage-14 Appendix A; stage 16 owns the
  closed `IntrinsicKind::Mmio(_)` vocabulary + Linux
  backend MMIO lowering + generic-C backend MMIO
  lowering decision (1 block).
- `../stage-17/expectations.md`: Atomic-acquire worked
  example dropped from stage-14 Appendix A; stage 17
  owns the closed `IntrinsicKind::Atomic(_)` vocabulary
  + Linux backend atomic lowering + generic-C backend
  atomic lowering (1 block).
