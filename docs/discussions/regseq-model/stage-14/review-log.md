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

---

## Cluster: D-14-4 (`render_const` composer renderer)

**Verdict: in-progress.** Phase A drafting complete
2026-05-13 with all 12 sub-decisions locked. Phase B
Codex review opened 2026-05-13. Round trail below.

Coverage: discussions.md D-14-4 main entry + 12
sub-decisions (D-14-4.1 API signature; D-14-4.2 scalar
variants; D-14-4.10 sentinel collapsing; D-14-4.11 hex
admission; D-14-4.12 float non-finite; D-14-4.3 ZST;
D-14-4.7 NullPtr; D-14-4.8 Reloc; D-14-4.4 Aggregate;
D-14-4.5 EnumValue; D-14-4.6 Bytes; D-14-4.9
type-context threading + operand-callsite wiring) +
pretty-printer-snapshot.md §10 Composer renderers
(§10.1.1-§10.1.8) + §7.2 render_operand signature
extension + §7.3 render_rvalue signature extension +
§6.7 render_generic_args signature extension + §8
ownership table + §9.10 specimens 76-109 + scattered
§1.3 / §3.3 / §6.4 / §9 specimen revisions + spec-snapshot.md
§9.7 bridge-interning invariant addition for
fat-pointer auxiliary types + prior-art/rustc-const.md
+ rustc-references.md "Const rendering" section.

### R1 -- 2026-05-13

**Verdict: BLOCK / Lockability: no / Close-out: NO.**

5 findings (3 BLOCK + 1 MINOR + 1 NIT):

1. **[BLOCK]** D-14-4.9's operand Ty threading not
   propagated through §7.3-§7.9 body renderers. §7.2
   updated to the pair-form `render_operand(&Operand,
   &Body, expected_ty: TyId)` but §7.3 (Rvalue::Use,
   Cast, BinaryOp), §7.4 (Assume, CopyNonOverlapping),
   §7.6 (SwitchInt), §7.7 (Indirect callees), §7.9
   (assert message operands) tables still called the
   two-arg form. **Resolution:** Updated every §7.3 /
   §7.3.2 / §7.4 / §7.6 / §7.7 / §7.9 row that calls
   `render_operand` to pass an explicit `expected_ty`,
   with an explicit derivation column added to each
   table. §7.4 Assign threads `place_ty(place, body)`
   to `render_rvalue(rvalue, body, expected_ty)`. §7.3
   gained a derivation column for per-operand
   `expected_ty` derivation. §7.3.2 AggregateKind table
   gained the analogous column. §7.4 Assume + CNO
   intrinsics pass explicit Tys (`Ty::Bool` for
   Assume cond; pointer + usize Tys for CNO src/dst/count).
   §7.6 SwitchInt threads `discr_ty` via operand
   inspection. §7.7 Indirect callee passes
   `Ty::FnPtr(sig_id)`. §7.9 assert-kind paragraph at
   §7.9 line ~917 rewritten to enumerate per-placeholder
   `expected_ty` derivation rules (BoundsCheck both
   `Ty::Uint(Usize)`; Overflow uses BinOp input;
   MisalignedPointerDereference both `Ty::Uint(Usize)`;
   InvalidEnumConstruction uses discriminant scalar
   type; etc.).

2. **[BLOCK]** `render_rvalue` lacks the context for
   `Rvalue::Use`'s `expected_ty`. The R1 finding
   identified that the REOPEN is necessary but
   incomplete: `Rvalue::Use(Operand)` carries only an
   Operand (per `spec-snapshot.md` §21 line ~7026),
   `Operand::Constant(ConstId)` carries no per-operand
   TyId (per `spec-snapshot.md` §20.1 line ~6730), and
   the current `render_rvalue(&Rvalue, &Body) ->
   String` signature has no expected-Ty input.
   **Resolution:** Extended the D-13-6 REOPEN to also
   revise `render_rvalue` to take an `expected_ty:
   TyId` parameter. Caller cascade pinned:
   `render_statement` Assign threads `place_ty(place,
   body)` -> `render_rvalue(_, _, expected_ty)` ->
   `render_operand(_, _, derived_operand_ty)`. §7.3
   signature line + ownership-table row + §8 row
   updated; D-14-4.9 discussion-side text updated to
   explain the cascade (3 stage-13 signatures now in
   REOPEN scope: `render_operand`, `render_rvalue`,
   `render_generic_args`).

3. **[BLOCK]** Fat-pointer fallback recursion's
   synthetic non-interned TyIds violate the closed-Ty
   + all-interned-TyId invariant per `spec-snapshot.md`
   §10. The fallback for `Ty::Ref(_, [non-u8 slice or
   dyn], _)` consumer Tys required per-child Tys
   (`Ty::Ptr(pointee, Not)`, `Ty::Uint(Usize)`), but
   the original D-14-4.9 spec said the renderer
   constructs these synthetic and "not interned into
   the TyInterner." This contradicts the closed-Ty
   model. **Resolution:** Replaced the "synthetic
   non-interned" rule with a **bridge interning
   invariant**: when the bridge produces
   `Const::Aggregate([Reloc, Uint])` for a fat-pointer
   consumer Ty per §9.7 dispatch line 3250 (`Slice {
   data, meta }`) or the analogous dyn-Trait synthesis
   paths, it MUST ensure both `Ty::Ptr(pointee,
   Mutability::Not)` and `Ty::Uint(UintKind::Usize)`
   are present in `model.types` (interning if absent).
   The renderer trusts the invariant and resolves TyIds
   via `TyInterner` lookup. `spec-snapshot.md` §9.7
   reference-typed-const-handling subsection gained
   the new "Bridge interning invariant for fat-pointer
   auxiliary types" paragraph at line ~3367; D-14-4.9
   discussion + §10.1.8 + §10.1.3 fat-pointer-fallback
   row + D-14-4.9 worked example all updated to drop
   "synthetic; not in TyInterner" wording in favor of
   "interned via TyInterner per bridge invariant."

4. **[MINOR]** `render_generic_args` signature
   couldn't satisfy the pair-form for
   `GenericArg::Const(c)`: the existing two-arg
   `render_generic_args(&GenericArgs)` had no access to
   the enclosing Item to resolve the declared types of
   const generic-args. **Resolution:** Extended the
   D-13-6 REOPEN to also revise `render_generic_args`
   to `render_generic_args(&GenericArgs, item: ItemId)
   -> String`. §6.7 signature line + helper docs
   updated; §6.4 ADT callsites (Struct / Enum / Union /
   Closure / FnDef) pass the adjacent `item`; §10.1
   composer-renderer callsites updated to pass `item`
   uniformly (all 13 `render_generic_args(args)`
   callsites in pretty-printer-snapshot.md updated to
   the new form via global replace; 13 callsites in
   discussions.md updated similarly).

5. **[NIT]** Locked §10.1 text contained stale
   "pending" wording at two spots: §10.1 line ~1909
   said subsections are "locked or pending"; §10.1.1
   Float row at line ~1939 said non-finite forms are
   specified by D-14-4.12 "(pending)." **Resolution:**
   Both edits applied to reflect LOCKED status (§10.1
   line says "all LOCKED at Phase A on 2026-05-13";
   §10.1.1 Float row points to the non-finite subsection
   below + says "(LOCKED)").

Codex prompt: `/tmp/codex_d144_r1.txt` (input via
heredoc).
Codex output: `/tmp/codex_d144_r1_output.txt`.
Codex lockability rationale (excerpt): "The pair-form
itself is justified: `Operand::Constant` lacks a TyId
at docs/discussions/regseq-model/stage-14/spec-snapshot.md:6795,
while type-dependent const arms are explicitly called
out at docs/discussions/regseq-model/stage-14/pretty-printer-snapshot.md:1844.
The blockers are propagation and TyId-validity issues,
not missing coverage of the 11 `Const` variants."

R2 verification will check that the cascade is
complete (no callsites left at two-arg form) and that
the bridge-interning invariant is consistent with the
rest of §9.7 dispatch and §3.3 validation.

### R2 -- 2026-05-13

**Verdict: BLOCK / Lockability: no / Close-out: NO.**

**R1 resolutions: 4/5 converged** -- R1.3 regression
identified in `status.md` recap text (two locked
recap entries still preserved the rejected
synthetic-non-interned framing for fat-pointer
fallback child TyIds, while
`spec-snapshot.md:3369-3385` and
`pretty-printer-snapshot.md:2466-2475` correctly
landed the bridge-interning invariant replacement).

3 findings (1 R1.3-regression + 1 MINOR + 1 NIT):

1. **[R1.3 REGRESSION]** `status.md:397` and
   `status.md:783-787` still described the
   fat-pointer fallback child TyIds as
   "synthetic-TyId construction" / "not interned into
   TyInterner -- renderer-internal helper only,"
   contradicting the R1.3 resolution. **Resolution:**
   Both recap entries rewritten in place to reference
   the bridge-interning invariant: the renderer
   resolves the auxiliary `Ty::Ptr(pointee, Not)` +
   `Ty::Uint(Usize)` TyIds via `TyInterner` lookup,
   per the bridge invariant added at
   `spec-snapshot.md` §9.7 line ~3367. Status entry
   at line 393-399 (Aggregate cluster recap) +
   line 770-808 (D-14-4.9 recap) both updated.

2. **[MINOR]** Cross-stage friction summaries did not
   consistently document the three-signature D-13-6
   cascade. `discussions.md:2367-2378` (D-14-4 main
   entry friction) and `status.md` at three locations
   (lines 19-25 header friction, 343-345 active focus
   friction, 696-705 D-14-4 cluster locked-decisions
   row, 1151-1170 cross-stage friction section) all
   mentioned only `render_operand`, while D-14-4.9
   discussion at `discussions.md:4513-4522` correctly
   listed all three signatures (`render_operand`,
   `render_rvalue`, `render_generic_args`).
   **Resolution:** Rewrote all four
   status-side friction summaries + the D-14-4 main
   entry friction paragraph to enumerate the
   three-signature cascade explicitly, citing R1
   finding as the source of the expansion. Added a
   second friction paragraph in the D-14-4 main entry
   for the §9.7 bridge-interning invariant (the
   second cross-stage friction surfaced by R1.3,
   which was previously not documented in the main
   entry).

3. **[NIT]** Obsolete one-arg `render_generic_args(args)`
   callsite at `status.md:894` in the D-14-4.3 ZST
   recap (`Ty::FnDef(item, args)` rendering form). The
   global replace in R1 caught 13 callsites in
   `pretty-printer-snapshot.md` + 13 in
   `discussions.md` but missed this one in
   `status.md`. **Resolution:** Edited line 894 to
   `<render_def_path(item)><render_generic_args(args, item)>`.

Codex prompt: `/tmp/codex_d144_r2.txt`.
Codex output: `/tmp/codex_d144_r2_output.txt`.

R3 verification will check that the R1.3 regression
is fully closed (no more "synthetic" / "not interned"
language in any cross-cluster recap) and that the
cross-stage friction summaries are now consistent
across `discussions.md`, `status.md`, and
`review-log.md`.

### R3 -- 2026-05-13

**Verdict: BLOCK / Lockability: no / Close-out: NO.**

**R1 resolutions: 4/5 converged** (R1.3 still
incomplete -- new locations surfaced).
**R2 resolutions: 3/3 converged for the specific R2
edits.** R3 found additional leftover text in
locations R2 didn't enumerate.

2 findings (1 R1.3-regression continuation + 1
MINOR):

1. **[R1.3 REGRESSION]** `status.md:470` (D-14-4.9
   recap, "synthetic TyIds (data ptr + usize) NOT
   interned") and `status.md:741` (D-14-4.4 row,
   "via D-14-4.9 synthetic-TyId construction") still
   carried the rejected framing. The R2 edits caught
   2 occurrences at the prior line numbers (397,
   783-787); R3 found these 2 additional occurrences
   that R2 missed (different line numbers because
   the file shifted from prior edits). **Resolution:**
   Both rewrites applied: line ~466-468 (D-14-4.9
   recap fat-pointer line) and line ~739-744
   (D-14-4.4 row fallback line) updated to reference
   bridge-interned auxiliary TyIds resolved via
   `TyInterner` lookup per the §9.7 bridge interning
   invariant. Final sweep across `status.md` /
   `discussions.md` / `pretty-printer-snapshot.md` /
   `spec-snapshot.md` returned no remaining
   "synthetic.TyId" / "NOT interned" / "renderer-
   internal helper" hits (excluding the
   `review-log.md` finding-text preservation).

2. **[MINOR]** Two remaining D-13-6 REOPEN summaries
   named only `render_operand`:
   - `pretty-printer-snapshot.md:2477` -- §10.1.8
     operand-callsite wiring intro showed only the
     `render_operand` before/after signature.
   - `pretty-printer-snapshot.md:2515` -- D-13-6
     merger annotation in §10.1.8 only recorded the
     `render_operand` extension.
   **Resolution:** Rewrote §10.1.8's operand-callsite
   wiring intro + the merger annotation to enumerate
   the three-signature cascade. The intro now shows
   BEFORE / AFTER for all three signatures
   (`render_operand`, `render_rvalue`,
   `render_generic_args`) plus an explanation of why
   `render_rvalue` + `render_generic_args` were forced
   to extend; the merger annotation lists all three
   signatures' final shapes. (`pretty-printer-snapshot.md`
   line ~2477-2545 area rewritten.) `status.md:471`
   was implicitly resolved by the line-470 D-14-4.9
   recap rewrite (which now mentions the cascade
   explicitly with all three signatures).

Codex prompt: `/tmp/codex_d144_r3.txt`.
Codex output: `/tmp/codex_d144_r3_output.txt`.
Codex lockability rationale (excerpt): "Lockability
is blocked only by stale recap/spec-summary text,
not by the core locked rules. Recommend a focused
R4 after these two edits."

R4 verification will check the exact-phrase sweep
(no "synthetic" / "not interned" anywhere outside
review-log.md) + the three-signature cascade
documentation in every D-13-6 REOPEN summary.

### R4 -- 2026-05-13

**Verdict: APPROVE / Lockability: yes / Close-out: YES.**

**R1 resolutions: 5/5 converged.** R1.3 bridge-interning
invariant is normative in `spec-snapshot.md:3369-3385`,
mirrored in `pretty-printer-snapshot.md:2466-2475`,
`discussions.md:4519-4533`, and status recaps at
`status.md:470-475` / `status.md:741-744`.

**R2 resolutions: 3/3 converged.** Three-signature
D-13-6 cascade present in header / status summaries
at `status.md:19-26`, `status.md:343-347`,
`status.md:723-725`; obsolete one-arg ZST recap
updated at `status.md:910-912`.

**R3 resolutions: 2/2 converged.** R3.1 locations now
use bridge-interned auxiliary TyIds via `TyInterner`
lookup at `status.md:470-475` and `status.md:741-744`;
R3.2's §10.1.8 intro and merger annotation now list
all three cascade signatures at
`pretty-printer-snapshot.md:2477-2490` and
`pretty-printer-snapshot.md:2535-2540`.

0 new findings.

Codex prompt: `/tmp/codex_d144_r4.txt`.
Codex output: `/tmp/codex_d144_r4_output.txt`.
Codex lockability rationale: "Three-signature cascade
consistency is clean across the requested locations:
main D-14-4 entry `discussions.md:2367-2403`,
D-14-4.9 friction `discussions.md:4553-4571`,
§10.1.8 `pretty-printer-snapshot.md:2477-2540`, and
cross-stage friction `status.md:1168-1203`. The
11-Const-variant renderer surface remains locked
across Scalar, ZST, Aggregate, EnumValue, Bytes,
NullPtr, and Reloc at
`pretty-printer-snapshot.md:1912-2393`. No R1/R2/R3
regression remains, the D-13-6 cascade is
consistently documented, exact stale callsite forms
are absent, and the §9.7 bridge-interning chain is
coherent. Recommend closing stage-14 D-14-4 cluster."

Cluster locked at R4.

### Trajectory summary

- 4 rounds total at per-stage Phase B level.
- 11 distinct findings closed:
  - **R1**: 3 BLOCK + 1 MINOR + 1 NIT (5 findings).
  - **R2**: 1 R1.3-regression + 1 MINOR + 1 NIT (3
    findings, plus 1 R1.3 regression-continuation).
  - **R3**: 1 R1.3-regression-continuation + 1 MINOR
    (2 findings; the R1.3 line carried over from R2's
    incomplete sweep).
  - **R4**: APPROVE / 0 findings.
- Load-bearing findings:
  - R1.1 (operand Ty threading not propagated through
    §7.3-§7.9): forced explicit derivation columns in
    every per-arm table.
  - R1.2 (`render_rvalue` lacks expected_ty context
    for `Rvalue::Use`): forced cascade expansion from
    1 signature to 3 signatures in the D-13-6 REOPEN.
  - R1.3 (fat-pointer fallback synthetic non-interned
    TyIds violate closed-Ty invariant): forced the
    bridge-interning invariant addition at
    `spec-snapshot.md` §9.7. **The longest-tail
    finding** -- carried through R2 + R3 because
    "synthetic" framing appeared in 4 distinct status
    + spec recap spots that progressively surfaced as
    R1's edit caught the canonical locations but
    missed the recaps. R4 confirmed the final sweep
    closed.
- Compared to prior-stage trajectories:
  | Stage cluster | Phase B rounds | Total findings |
  | ------------- | -------------- | -------------- |
  | Stage 14 D-14-1+D-14-3 | 3 | 8 (1B + 4M + 3N) |
  | Stage 14 D-14-2 | 6 | 14 (0B + 11M + 3N) |
  | Stage 14 D-14-10 | 4 | ~24 (1B + ~17M + ~6N at
    R1; collapsed via lift+light hints) |
  | **Stage 14 D-14-4** | **4** | **11 (3B + 4M + 2N + 2 regression-continuations)** |
- The cluster's trajectory reflects: (a) the
  composer-renderer cascade was deeper than initial
  D-14-4.9 spec captured -- `render_rvalue` and
  `render_generic_args` signature extensions were
  load-bearing follow-ups; (b) the fat-pointer
  fallback TyId story needed grounding in the
  `TyInterner` closed-system invariant, surfacing a
  small bridge-side invariant addition at §9.7; (c)
  the regression-continuation pattern (R1.3 carrying
  through R2 + R3) was driven by recap-text
  duplication across 4-5 status / spec locations
  that the initial edit caught some-but-not-all of.
  No load-bearing architectural decisions were
  revised at Phase B; the 12 sub-decision rules
  themselves all held.

---

## Cluster: D-14-5 (`render_instance` composer renderer)

**Verdict: in-progress.** Phase A drafting complete
2026-05-13 with all 8 sub-decisions LOCKED + 19
specimens added at `pretty-printer-snapshot.md`
§9.11. Phase B Codex review opened 2026-05-13.
Round trail below.

Coverage: `discussions.md` D-14-5 cluster opening
(Question + Evidence basis + Sub-decision allocation
+ Spec impact preview + Methodology rules invoked +
Phase A drafting plan + Status) + 8 sub-decision
audit-shape subsections (D-14-5.1 API signature;
D-14-5.2 Function variant envelope; D-14-5.3 Static
variant; D-14-5.4 Const variant; D-14-5.5 ADT
variants Option B; D-14-5.6 attribute block
composition; D-14-5.7 layering boundary; D-14-5.8
cross-stage friction outcome) + `pretty-printer-snapshot.md`
§1.3 composer listing + §5.6 shared helper
subsection + §8 ownership table + §10.2 intro +
§10.2.1-§10.2.7 per-variant + attribute-composition
subsections + §9.11 specimens 110-128 + §11
cross-reference + `spec-snapshot.md` §9.4
Consumer-side-rendering paragraph +
`prior-art/rustc-instance.md` (10 Q-tags) +
`rustc-references.md` Instance-rendering section.

### R1 -- 2026-05-13

**Verdict: BLOCK / Lockability: no / Close-out: NO.**

7 findings (3 BLOCK + 3 MINOR + 1 NIT):

1. **[BLOCK]** §5.6 helper row-order recap +
   D-14-5.6 audit-shape listing diverged from the
   locked §5.3 table-row order: my edits placed
   `link_section` *before* lang-flags, but the
   locked §5.3 table at
   `pretty-printer-snapshot.md:414` has the inverse
   order (`is_naked` -> lang-flags -> `link_section`
   -> `opaque_handle_eligible`). Snapshot-stability
   + cross-stage-friction issue against D-14-5.6 /
   D-14-5.7: the helper claims to preserve D-13-4.3
   unchanged while reordering load-bearing rows.
   **Resolution:** Both §5.6 helper row-order
   recap (`pretty-printer-snapshot.md:558` ff)
   and D-14-5.6 audit-shape listing
   (`discussions.md:6599` ff) edited in place to
   match the locked §5.3 sequence: 1. linkage;
   2. inline_hint; 3. is_naked; 4. lang-flags
   (phantom_data / unsafe_cell / manually_drop);
   5. link_section; 6. opaque_handle_eligible.
   Specimen 112 annotation
   (`pretty-printer-snapshot.md:2064`) summarises
   only the rows surviving suppression for that
   specimen (linkage + inline + link_section) and
   stays consistent with the corrected helper
   sequence; no edit needed there.

2. **[BLOCK]** Specimen 118 + §10.2.2 Static worked
   example + D-14-5.3 audit-shape worked example
   rendered the initializer value as
   `0x10009000_u32` (hex form), but D-14-4.11 LOCKS
   `render_const` `Uint` rendering at **decimal-only
   for v1** (per the §10.1.1 Uint table at
   `pretty-printer-snapshot.md:1933` and the locked
   "no hex form admission" rule). Static delegates
   initializer text to the locked D-14-4
   `render_const(c, ty)` composer per D-14-5.3, so
   the specimen + examples must obey the same rule.
   **Resolution:** Replaced 5 occurrences of
   `0x10009000_u32` with the decimal form
   `268472320_u32` (= 0x10009000 in decimal) across
   `pretty-printer-snapshot.md` (specimen 118 input
   + output + annotation lines 2182, 2186, 2189;
   §10.2.2 worked example line 3467) and
   `discussions.md` (D-14-5.3 worked example line
   5886). No hex-admission exception added to
   D-14-5.

3. **[BLOCK]** D-14-5.2 composition outline
   (definition form) at `pretty-printer-snapshot.md:3286`
   and `discussions.md:5361` included a
   `let mut _1: T1;            // arg-local 1`
   declaration line, but §3.1 / stage-13 D-13-2
   LOCKED specifies that argument locals
   (`_1..=_arg_count`) appear ONLY in the function
   header arg-list and are NOT redeclared in the
   locals block (the §3.1 example at
   `pretty-printer-snapshot.md:184-187` shows `_0`
   then `_3`, `_4`, ...; the D-14-5 specimens
   110-117 all show `_0` then `...`). The outline
   would double-render arg-locals if followed
   literally. **Resolution:** Removed the
   `let mut _1: T1; ... let _M: T` two-line
   sketch and replaced with a single non-arg local
   line (`let mut _M: T; // first non-arg local
   (M == arg_count + 1)`) plus an inline comment
   under `let mut _0: <ret>;` explaining that
   arg-locals `_1..=_N` are declared in the
   header arg-list above and not redeclared per
   §3.1 / D-13-2. Applied to both
   `pretty-printer-snapshot.md` §10.2.1 and
   `discussions.md` D-14-5.2 audit-shape.

4. **[MINOR]** D-14-5.2 declaration-form trailer
   table row + D-14-5.2 audit-shape narrative said
   the `;` lands "immediately after the closing `)`
   of arg-list" but D-14-5.2 aspect 4 LOCKS
   always-render `-> Ret`, so the `;` is after the
   return-type segment, not after the arg-list
   closing paren. The worked examples correctly
   render `extern "C" fn libc::printf(_1: *const u8) -> i32;`
   (semicolon after `i32`). **Resolution:**
   Edited the table row at
   `pretty-printer-snapshot.md:3258` to
   "`;` (immediately after the return-type segment
   `-> <ret>`)"; edited the narrative at
   `discussions.md:5347` to add an explanatory
   sentence pinning the trailer position to the
   end of the header line after the return-type
   segment, with a back-pointer to D-14-5.2
   aspect 4.

5. **[MINOR]** Specimen 112's structured input had
   `Item.link_section = Some("\".text.boot\"")`
   with escaped quotes inside the stored string,
   but the rendering rule expects the stored
   string to be the section name (without
   surrounding quotes) and the renderer adds the
   quotes as part of the `#[link_section = "..."]`
   form. As written, the structured input would
   imply a stored value of `".text.boot"`
   (with literal quote chars), and the rendered
   output would be
   `#[link_section = "\".text.boot\""]` -- not the
   expected `#[link_section = ".text.boot"]`.
   **Resolution:** Edited the specimen input at
   `pretty-printer-snapshot.md:2046` to
   `Some(<StringId resolving to ".text.boot">)`,
   making explicit that the stored value is the
   section name and the rendering form adds the
   quotes.

6. **[MINOR]** Stale close-out text:
   `pretty-printer-snapshot.md` §11 cross-reference
   row for "§9 Instance / InstanceKind" at line
   3930 still read "stage-14's `render_instance`
   once shipped (§10.2)", although §10.2 is now
   fully populated. The D-14-5 §3 status-summary
   entry at `discussions.md:7647` still listed
   "~10-15 specimens" as remaining work even
   though 19 specimens had landed at §9.11.
   **Resolution:** Retargeted §11 row to point at
   §10.2 with the §10.2.1-§10.2.7 sub-subsection
   inventory; updated the §3 status-summary entry
   to record "19 specimens (110-128) landed at
   §9.11" + flipped "remaining work" to "Phase B
   Codex review (R1 closed 2026-05-13 at BLOCK
   with 3B + 3M + 1N findings; resolutions
   applied; R2 next)".

7. **[NIT]** §5.6 helper subsection + §8 ownership
   table row for `render_item` + D-14-5 cluster
   opening + D-14-5.7 audit-shape used the wording
   "byte-identical attribute streams" without
   qualification, but per D-14-5.6 / D-14-5.7
   `render_item` consumes only Stream 1 (Item-flag)
   from the helper; Stream 2 (`domain_attrs.item_attrs`)
   lives at the Instance-level composer only.
   The original wording is technically overbroad
   even though the §5.6 subsection clarifies
   "Sources NOT consumed" immediately after. The
   architecture is correct; the wording can mislead.
   **Resolution:** Qualified 5 occurrences to
   "byte-identical Stream 1 (Item-flag) attribute
   streams" across `pretty-printer-snapshot.md`
   §5.6 + §8 `render_item` row + §10.2 layering
   paragraph and `discussions.md` D-14-5 cluster
   opening + D-14-5.7 audit; added an explicit
   "`render_item` does NOT emit Stream 2
   domain-attribute rows" sentence at the load-
   bearing spots so the boundary cannot be misread.

**Round closure note.** All 7 R1 findings closed at
this pass; resolutions land in
`pretty-printer-snapshot.md` §5.6 + §8 + §10.2.1 +
§10.2.2 + §10.2.7 + §11 + Specimen 112/118
inputs/outputs and `discussions.md` D-14-5.2 +
D-14-5.6 + D-14-5.7 audit-shape entries + §3
status-summary. No stage-13 D-tag REOPENs introduced
by the R1 fixes (the §5.3 row-order recap correction
in §5.6 is a recap-text alignment to the locked
table, not a table modification). Cluster proceeds
to R2 review.

### R2 -- 2026-05-13

**Verdict: BLOCK / Lockability: no / Close-out: NO.**

4 findings (1 BLOCK + 2 MINOR + 1 NIT). 3
continuations of R1 (R1.1, R1.6, R1.7) + 1 new
finding surfaced by the per-variant cross-reference
audit:

1. **[BLOCK] [R1.1 continuation]** §10.2 dispatch
   overview at `pretty-printer-snapshot.md:3084-3086`
   listed Stream 1 fields in the order `linkage /
   inline_hint / is_naked / lang-flags /
   opaque_handle_eligible / link_section`, but the
   locked §5.3 table has `link_section` BEFORE
   `opaque_handle_eligible`. R1.1 fixed the §5.6
   helper + D-14-5.6 audit row-order recaps but
   missed this third location -- the active §10.2
   intro that downstream readers of `render_instance`
   spec encounter first. **Resolution:** Rewrote
   the §10.2 intro parenthetical at
   `pretty-printer-snapshot.md:3084` to avoid
   re-listing the field order (which has no
   independent authority on order) and instead
   pointed at §5.3 + §5.6 for the authoritative
   Stream 1 field enumeration + walk order.
   Removes the third source of truth and prevents
   future drift.

2. **[MINOR] [R1.6 continuation]** Multiple stale
   "pending/deferred" wordings surfaced in the
   audit-shape narrative that R1.6 didn't catch:
   - Cluster opening at `discussions.md:4685-4687`
     described §10.2 as a "(Pending D-14-5 lock.)"
     stub (true at allocation; stale now that
     §10.2.1-§10.2.7 are LOCKED).
   - D-14-5.3 attribute-block aspect at
     `discussions.md:5715` said composition rule
     "deferred to D-14-5.6 (pending)" (D-14-5.6 is
     LOCKED).
   - D-14-5.4 attribute-block aspect at
     `discussions.md:5944` said "composition rule
     deferred to D-14-5.6" (LOCKED).
   - D-14-5.5 ADT common-decoration at
     `discussions.md:6177` said "composition rule
     deferred to D-14-5.6" (LOCKED).
   - D-14-5.8 Next steps at
     `discussions.md:7524-7541` still listed
     "Specimens (Step 8). Add ~10-15 specimens..."
     as work-to-do (19 specimens 110-128 ARE
     landed; tracked at the cluster-opening Status
     block).
   - D-14-5.2 worked-examples narrative at
     `discussions.md:5392-5396` said "the precise
     ordering ... is finalised at D-14-5.6 lock"
     (LOCKED; should refer to LOCKED state).
   - Code-comment in the D-14-5.2 illustrative
     intrinsic example at `discussions.md:5653-5655`
     said "attribute-block ordering finalises at
     D-14-5.6 lock" (LOCKED).
   - Same illustrative-example code-comment in
     `pretty-printer-snapshot.md:3358-3360`
     (mirrors discussions.md:5653-5655).
   **Resolution:** Edited each location to reflect
   the LOCKED state: D-14-5.3/D-14-5.4/D-14-5.5
   attribute-block aspects now say "composition
   rule pinned at §10.2.7 per D-14-5.6 LOCKED";
   cluster opening describes §10.2 as "introduced
   as a stub at D-14-4 lock per the §10 Composer
   renderers framing; subsequently populated by
   this cluster with §10.2.1-§10.2.7 per-variant +
   attribute-composition subsections at the
   respective sub-decision locks"; D-14-5.8 Next
   steps flips Step 8 to "DONE" with the 19
   specimens enumerated, Step 9 flips to "IN
   PROGRESS" with R1 / R2 status tracked;
   D-14-5.2 narrative + illustrative-example
   code-comments updated to describe the locked
   ordering (Stream 1 emitted first via §5.6
   helper; Stream 2 emitted after) instead of
   "finalises at D-14-5.6 lock". The historical
   "aspect updated from 'composition rule
   deferred'" wording at
   `discussions.md:6864` is in D-14-5.6's "Spec
   impact" describing the BEFORE state of the
   D-14-5.6 lock's text-transition; that's
   accurate historical narrative, not stale text,
   no edit.

3. **[MINOR] [new]** §10.2.4 Struct, §10.2.5 Enum,
   §10.2.6 Union per-variant subsections used
   `[<attribute-block>\n]` in their composition
   forms but did not carry the explicit "Attribute
   block" aspect cross-reference to §10.2.7 + §5.6
   that §10.2.1 / §10.2.2 / §10.2.3 (Function /
   Static / Const) carry. The composition rule is
   recoverable from §10.2.7 (since the rule is
   common across all variants), but the per-variant
   cross-reference coverage was incomplete and
   inconsistent with the per-variant subsection
   pattern. **Resolution:** Added a short common
   "Attribute block" paragraph to §10.2.4
   (`pretty-printer-snapshot.md` after the
   intro paragraph, before "Dispatch on
   (fields.len(), ...)"), §10.2.5 (after the intro
   paragraph, before "Dispatch on shape: EnumShape"),
   and §10.2.6 (after the intro paragraph, before
   "**Form.** Unions carry only named-field
   bodies..."). Each says: "Per §10.2.7
   attribute-block composition rule (D-14-5.6
   LOCKED): Stream 1 (Item-flag, §5.3 table-row
   order via the §5.6 shared helper) precedes
   Stream 2 (`domain_attrs.item_attrs[item]`
   Vec-index order). Stage-14 ships no
   `AttrData::Intrinsic` variant occupying [Struct
   / Enum / Union] Items in v1 ...; the attribute
   block reduces to Stream 1 in v1." §10.2.6 Union
   subsection adds an extra sentence calling out
   `#[lang = "unsafe_cell"]` as a common Stream 1
   entry for Union Items (e.g.,
   `core::cell::UnsafeCell`).

4. **[NIT] [R1.7 continuation]** `status.md:181-186`
   in a previous status entry (the D-14-5.6 +
   D-14-5.7 lock recap) still used unqualified
   "byte-identical attribute streams" -- the
   immediately-preceding sentence in the same
   paragraph said the helper produces Stream 1, so
   the risk is low. R1.7 caught the current top
   status entry + cluster-opening wording but
   missed the prior status entry. **Resolution:**
   Qualified the wording at `status.md:186` to
   "byte-identical Stream 1 (Item-flag) attribute
   streams; `render_item` does NOT emit Stream 2
   domain-attribute rows".

**Round closure note.** All 4 R2 findings closed
at this pass; resolutions land in
`pretty-printer-snapshot.md` §10.2 intro + §10.2.4
+ §10.2.5 + §10.2.6 + illustrative-example
code-comment in §10.2.1 examples block;
`discussions.md` cluster opening + D-14-5.2
worked-examples narrative + illustrative-example
code-comment + D-14-5.3 / D-14-5.4 / D-14-5.5
attribute-block aspects + D-14-5.8 Next steps; and
`status.md:186`. **No stage-13 D-tag REOPENs**
introduced by R2 fixes; the §10.2 intro
parenthetical rewrite is recap-text alignment (the
locked §5.3 table content + row order remain
unchanged). The R1+R2 pattern matches D-14-4's
recap-text-duplication phenomenon: R1 caught the
canonical locations + R2 caught spots in
audit-shape narrative + illustrative-example
code-comments + status.md recap that R1 missed.
The new R2.3 finding (per-variant attribute-block
cross-reference coverage) was a spec-hygiene gap
not surfaced at R1, surfaced cleanly by the R2
audit. Cluster proceeds to R3 review.

### R3 -- 2026-05-13

**Verdict: MINOR / Lockability: yes / Close-out: NO.**

2 findings (0 BLOCK + 2 MINOR + 0 NIT). 1
continuation of R2 (R2.2 -- five more
stale "defers/will pin/may/in progress"
wordings in the audit narrative) + 1 new finding
(§3.3 sigil table contradicts D-14-5.7
composer-from-scratch). The cluster crossed
Lockability:yes at R3: BLOCK findings are exhausted,
no specimen correctness issues, no stage-13
REOPENs, architectural picture holds. Close-out
withheld pending these two text cleanups.

1. **[MINOR] [R2.2 continuation]** Five more
   D-14-5 audit narrative spots still described
   locked work as future work:
   - `discussions.md:5311` -- D-14-5.2 Aspect 1
     said "D-14-5.2 defers the internal
     composition rule ... to D-14-5.6" (LOCKED
     since D-14-5.6 lock).
   - `discussions.md:5598` -- D-14-5.2 spec-impact
     note said "D-14-5.7 layering lock will pin
     whether `render_instance` calls `render_item`
     or composes from scratch" (D-14-5.7 LOCKED;
     composer-from-scratch chosen).
   - `discussions.md:6720` -- D-14-5.6 Reasoning
     point 2 said "modulo D-14-5.7's layering
     boundary -- which may share the §5.3 helper
     or call `render_item` directly" (D-14-5.7
     LOCKED at the §5.6 shared helper).
   - `discussions.md:7551` -- D-14-5.8 Next steps
     said "R2 in progress" (R2 closed; R3 in
     progress).
   - `discussions.md:7675` -- §3 status-summary
     entry said "R2 next" (R2 closed; R3 next at
     that point, R4 next now).
   **Resolution:** Updated each spot to
   locked-state wording:
   - D-14-5.2 Aspect 1: "The composition rule ...
     is pinned at §10.2.7 per D-14-5.6 LOCKED."
   - D-14-5.2 spec-impact: "Per D-14-5.7 LOCKED,
     `render_instance` composes from scratch via
     the §5.6 shared `render_item_attribute_block`
     helper (factored from §5.3 + §5.5) and does
     NOT call `render_item`."
   - D-14-5.6 Reasoning point 2: "Per D-14-5.7
     LOCKED, both renderers consume the §5.6 shared
     `render_item_attribute_block` helper that
     walks §5.3 in row order, guaranteeing
     byte-identical Stream 1 output."
   - D-14-5.8 Next steps: "R1 closed ... R2 closed
     ... R3 closed ... All R1-R3 resolutions
     applied; no stage-13 REOPENs introduced. R4
     close-out round anticipated next."
   - §3 status-summary: "R1 closed 2026-05-13 at
     BLOCK with 3B+3M+1N findings; R2 closed
     2026-05-13 at BLOCK with 1B+2M+1N findings;
     R3 closed 2026-05-13 at MINOR with 0B+2M+0N
     findings, Lockability:yes; all R1-R3
     resolutions applied; R4 close-out round next."

2. **[MINOR] [new]** §3.3 sigil convention table
   `InstanceId` row at `pretty-printer-snapshot.md:245`
   said `render_instance` "composes `render_item`
   wrapper + per-Instance content", contradicting
   the D-14-5.7 LOCKED composer-from-scratch
   architecture. The §3.3 row is a canonical
   pretty-printer table and could mislead
   implementers about the call graph despite §1.3
   (`pretty-printer-snapshot.md:100`), §8
   (`pretty-printer-snapshot.md:1144`), and §10.2
   (`pretty-printer-snapshot.md:3157` ff) being
   correct. The row had originally been written at
   stage-13 D-13-4.6's composer-listing time as a
   forward-pointer; R1 / R2 fixed §1.3 + §8 + §10.2
   when D-14-5.7 locked but missed the §3.3 row.
   **Resolution:** Updated the row to: "per
   D-14-5.7 LOCKED, composes `render_def_path` +
   per-`InstanceKind` content + the §5.6 shared
   `render_item_attribute_block` helper for
   Stream 1; does NOT call `render_item`".

**Round closure note.** All 2 R3 findings closed at
this pass; resolutions land in `discussions.md`
D-14-5.2 Aspect 1 + D-14-5.2 spec-impact + D-14-5.6
Reasoning point 2 + D-14-5.8 Next steps + §3
status-summary entry, and
`pretty-printer-snapshot.md` §3.3 sigil table
`InstanceId` row. **No stage-13 D-tag REOPENs**
introduced; the §3.3 row update finalises a
stage-13 D-13-4.6 forward-pointer (not a REOPEN of
locked stage-13 content). The R1+R2+R3 trajectory
matches the D-14-4 cluster's recap-text-duplication
pattern: each round catches additional spots that
the prior round missed in the audit-shape narrative
+ sigil-convention table + forward-pointer
wording. The cluster crossed Lockability:yes at R3
with BLOCK findings exhausted; close-out is on
track for R4 if R3 fixes verify clean and no new
findings surface. Cluster proceeds to R4
close-out review.

### R4 -- 2026-05-13

**Verdict: APPROVE / Lockability: yes / Close-out: YES.**

Zero findings. Cluster ready for Phase B close-out.

R3 resolutions verified clean across all surfaces:
- D-14-5.2 Aspect 1 + spec-impact at
  `discussions.md:5311` / `:5597` now use
  locked-state wording for the attribute-block
  composition + helper-call-path.
- D-14-5.6 Reasoning point 2 at
  `discussions.md:6714` no longer carries the
  "modulo D-14-5.7 ... may share or call directly"
  hedge.
- D-14-5.8 Next steps + §3 status-summary at
  `discussions.md:7533` / `:7652` record the full
  R1+R2+R3 trajectory.
- §3.3 sigil convention `InstanceId` row at
  `pretty-printer-snapshot.md:245` now matches §1.3
  (line 100), §8 (line 1144), and §10.2 (line 3156):
  composer-from-scratch with the §5.6 shared
  Stream 1 helper, no `render_item` call.
- §10.2 subsection map + cluster status across
  `discussions.md:4984`, `pretty-printer-snapshot.md:3138`,
  and `status.md:3` all reflect all 8 sub-decisions
  LOCKED.
- §5.3 per-flag table content + row order at
  `pretty-printer-snapshot.md:414` ff verified
  unchanged from stage-13 D-13-4.3 LOCKED; no
  stage-13 REOPEN introduced by any R1-R3 fix.
- D-14-5.8's zero-new-friction record verified at
  `discussions.md:7362` / `:7436` / `:7488`.

**Cluster locked at R4 close-out.** D-14-5 Phase B
APPROVE / Close-out:YES at 2026-05-13.

### Trajectory summary

- 4 rounds total at per-stage Phase B level.
- 13 distinct findings closed across R1-R3:
  - **R1**: 3 BLOCK + 3 MINOR + 1 NIT (7 findings).
  - **R2**: 1 BLOCK + 2 MINOR + 1 NIT (4 findings;
    3 continuations of R1 + 1 new).
  - **R3**: 0 BLOCK + 2 MINOR + 0 NIT (2 findings;
    1 continuation of R2 + 1 new).
  - **R4**: APPROVE / 0 findings.
- Load-bearing findings:
  - R1.1 (§5.6 helper row-order recap + D-14-5.6
    audit listed `link_section` before lang-flags
    vs locked §5.3 inverse order): forced the
    recap-text alignment + cascade through R2.1
    (§10.2 intro third source of truth) before
    settling at "point at §5.3 + §5.6 for
    authoritative order".
  - R1.2 (Static specimen 118 + worked examples
    used hex `0x10009000_u32` vs D-14-4.11
    decimal-only): forced 5 occurrences across 2
    files to decimal `268472320_u32`. Reinforced
    that Static / Const composers MUST obey
    D-14-4's locked `render_const` rules without
    exception.
  - R1.3 (D-14-5.2 composition outline showed
    arg-locals redeclared in the locals block,
    contradicting §3.1 / D-13-2 LOCKED): forced
    outline rewrite to show `_0` (RETURN_PLACE)
    then `_M` (first non-arg local) with inline
    §3.1 / D-13-2 reference comment.
  - R3.2 (§3.3 sigil convention `InstanceId` row
    contradicted D-14-5.7 composer-from-scratch
    architecture): stage-13 D-13-4.6 forward-pointer
    that R1 / R2 fixed at §1.3 + §8 + §10.2 but
    missed at §3.3. Surfaced cleanly at R3.
- Compared to prior-stage trajectories:
  | Stage cluster | Phase B rounds | Total findings |
  | ------------- | -------------- | -------------- |
  | Stage 14 D-14-1+D-14-3 | 3 | 8 (1B + 4M + 3N) |
  | Stage 14 D-14-2 | 6 | 14 (0B + 11M + 3N) |
  | Stage 14 D-14-10 | 4 | ~24 (1B + ~17M + ~6N) |
  | Stage 14 D-14-4 | 4 | 11 (3B + 4M + 2N + 2 reg-cont) |
  | **Stage 14 D-14-5** | **4** | **13 (4B + 7M + 2N; 5 R1/R2/R3 continuations + 8 fresh)** |
- The cluster's trajectory reflects the
  recap-text-duplication pattern (matching D-14-4):
  R1 caught canonical locations; R2 caught the
  §10.2 intro third source of truth + audit-shape
  narrative "deferred" wordings + §10.2.4/5/6
  cross-reference gap; R3 caught the §3.3 sigil
  row + 5 more audit-narrative spots. R4 APPROVE
  confirmed the cascade closed. No load-bearing
  architectural decisions were revised at Phase B;
  the 8 sub-decision rules themselves all held.
  No stage-13 D-tag REOPENs introduced (the §5.6
  helper factor-out was additive, not a §5.3 / §5.5
  table modification).

## Cluster: D-14-6 (top-level `render(&self)` composer renderer)

**Verdict: in-progress.** Phase A drafting complete
2026-05-13 with all 6 sub-decisions LOCKED (D-14-6.1
API signature; D-14-6.2 iteration order; D-14-6.3
per-Instance separator; D-14-6.4 file-level envelope;
D-14-6.5 empty-model case; D-14-6.6 cross-stage
friction outcome) + 5 specimens (129-133) added at
`pretty-printer-snapshot.md` §9.12. Phase B Codex
review opened 2026-05-13. Round trail below.

Coverage: `discussions.md` D-14-6 cluster opening
(Question + Evidence basis + Sub-decision allocation
+ Spec impact preview + Methodology rules invoked +
Phase A drafting plan + Phase B expectations) + 6
sub-decision audit-shape subsections + `pretty-
printer-snapshot.md` §1.3 composer listing + §8
ownership table row + §10.3.1-§10.3.7 sub-decision
spec subsections + §9.12 specimens 129-133 + `prior-
art/rustc-render.md` (6 Q-tags) + `rustc-references.md`
Top-level render section.

### R1 -- 2026-05-13

**Verdict: BLOCK / Lockability: no / Close-out: NO.**

6 findings (2 BLOCK + 4 MINOR + 0 NIT):

1. **[BLOCK]** Separator algorithm relied on an
   implicit `\n` trailing each `render_instance(id)`
   output, but the inherited §9.11 specimens render
   `render_instance` outputs as single-line forms
   (e.g., `static pl011::BASE_ADDR: u32 =
   268472320_u32;`) with no specified trailing
   newline. Under the inherited shape, `buf.push('\n')`
   between consecutive non-newline-terminated
   strings would produce zero blank lines, not the
   promised single blank line.
   **Resolution:** Re-architected §10.3.3 + §10.3.6
   to make `render(&self)` own the separator bytes:
   each `render_instance(id)` output is no-trailing-
   newline (per the §9.11 specimen contract);
   `render` emits the two-byte sequence `"\n\n"`
   between consecutive Instances (first `\n` ends
   the prior Instance's last line, second `\n`
   produces the single blank). §10.3.6 composition
   outline updated: `buf.push_str("\n\n")` replaces
   `buf.push('\n')`. §10.3.4 wording updated to
   reflect that `render` does not inject any trailing
   newline after the last Instance. **No
   `render_instance` reopen needed** -- D-14-5's
   signature + per-variant rules unchanged; the
   §9.11 specimens already establish the
   no-trailing-newline contract.

2. **[BLOCK]** §10.3.4 envelope rule said output
   begins with "the keyword line", but §10.2.7's
   attribute-block composition rule means the first
   rendered line is an attribute line
   (`#[rseq::intrinsic(...)]`, `#[foreign]`,
   `#[lang = "..."]`) whenever the Stream-1 +
   Stream-2 composition is non-empty. Falsified by
   Specimen 132 (first rendered line is
   `#[rseq::intrinsic("log-info")]`, not `fn`).
   **Resolution:** §10.3.4 wording updated to: "The
   output begins at column 0 of line 1 with the
   first rendered byte of the first Instance's
   `render_instance` output -- the first byte of
   its attribute block when §10.2.7's Stream-1 +
   Stream-2 composition produces a non-empty block,
   otherwise the first byte of the kind-keyword
   line." Symmetric tail wording applied for the
   last rendered byte.

3. **[MINOR]** §9.12 specimens and §10.3
   sub-sections carried D-tag references and
   "LOCKED" audit-history phrasing inconsistent
   with README §5.9 rule 5 (D-tag inline references
   avoided in spec text). Locations: §9.12 intro
   "per §10.3 D-14-6"; specimen annotations
   "D-14-6.5 LOCKED"; §10.3 intro "final composer
   in the stage-14 trio"; etc.
   **Resolution:** Stripped D-tag references and
   "LOCKED" phrasing from §9.12 specimens (129-133)
   + §10.3 intro + §10.3.6 composer description.
   Replaced with live cross-references ("per §10.3",
   "per §10.3.5", "per §10.2.7's attribute-block
   composition rule").

4. **[MINOR]** `discussions.md` D-14-6.3 and
   D-14-6.4 worked examples rendered the Static
   initializer as `0x10009000_u32` (hex), but
   D-14-4.11 LOCKS `render_const` Uint rendering
   at decimal-only.
   **Resolution:** Replaced both occurrences with
   `268472320_u32` (decimal form matching specimens
   118 + 131).

5. **[MINOR]** `prior-art/rustc-render.md` Q2 and
   `rustc-references.md` summary overstated
   `MonoItem::items_in_deterministic_order` as
   "sorts by `(Option<Span>, SymbolName)` with
   span first". Verified against the pinned source
   at `rustc_middle/src/mir/mono.rs:553-573`: the
   span-bearing `ItemSortKey` path is gated on
   `-Z codegen-source-order`; the default path
   sorts by `SymbolName` only.
   **Resolution:** Both summaries rewritten to
   document the two modes (default symbol-name
   only; `-Z codegen-source-order` -> span +
   symbol-name). Kept the supporting-evidence
   framing; non-load-bearing for D-14-6.2.

6. **[MINOR]** `prior-art/rustc-render.md` Q5
   empty-`mir_keys` example said "crates with
   only constants + type definitions can produce
   empty `mir_keys`", but Q1 already documented
   the trivial-const fast path -- const items DO
   appear in `mir_keys` and produce body content.
   **Resolution:** Replaced the example with
   type-only / foreign-only crates after the
   `GlobalAsm` filter at
   `rustc_mir_transform/src/lib.rs:334`. Const
   items explicitly noted as NOT empty-case
   triggers.

All 6 R1 findings closed at this pass (2 BLOCK +
4 MINOR + 0 NIT) across `pretty-printer-snapshot.md`
§9.12 + §10.3.3 + §10.3.4 + §10.3.6 + §10.3 intro
+ Specimen annotations 129-133 inputs/outputs;
`discussions.md` D-14-6.3 + D-14-6.4 worked
examples + D-14-6 audit cluster (no D-tag
reframings); `prior-art/rustc-render.md` Q2 + Q5;
`rustc-references.md` Top-level render section.

**Next concrete action**: kick off Phase B R2 Codex
review on the corrected D-14-6 spec contributions
(verify R1 fixes land; check for cascading findings
per the recap-text-duplication pattern observed in
D-14-4 / D-14-5).

### R2 -- 2026-05-13

**Verdict: MINOR / Lockability: yes / Close-out: NO.**

3 findings (0 BLOCK + 3 MINOR + 0 NIT) -- exactly the
recap-text-duplication cascade pattern anticipated.
Cluster crossed Lockability:yes at R2: BLOCK findings
exhausted; no specimen correctness issues; no stage-13
REOPENs; architectural picture holds.

1. **[MINOR]** Stale separator algorithm in audit
   narrative. R1's canonical §10.3 fix landed
   (`render_instance(id)` no trailing newline + top-
   level composer owns `"\n\n"` separator), but the
   D-14-6 audit trail in `discussions.md` still
   described the old model in multiple places: the
   D-14-6.1 worked-example sketch used
   `buf.push('\n')`; D-14-6.3 audit narrative said
   each `render_instance` owns implicit trailing-
   newline behavior; the visual-effect paragraph
   explained prior trailing `\n` + separator `\n`;
   the Methodology rules paragraph still rejected a
   no-trailing-newline hedge. `prior-art/rustc-render.md`
   Q3 also still framed the no-separator concatenation
   alternative as relying on each Instance's own
   trailing newline.
   **Resolution:** Updated the D-14-6.1 sketch to
   `buf.push_str("\n\n")` with a multi-line comment
   explaining the byte-level semantics. Rewrote
   D-14-6.3's Decision / Reasoning / Rejected
   alternatives / Methodology rules / Spec impact
   paragraphs to say RegseqModel adapts rustc's
   first-flag pattern but owns both separator bytes
   because per-Instance outputs end at `}` or `;`.
   Updated `prior-art/rustc-render.md` Q3 to describe
   the (A) recommendation as "composer-owned `"\n\n"`
   separator" and the (C) no-separator alternative as
   "places the next Instance's first byte directly
   after the prior Instance's closing byte on the
   same line" (rather than the prior wording about
   implicit per-Instance newlines).

2. **[MINOR]** Stale keyword-line / trailing-tail
   envelope wording outside the canonical §10.3.4.
   R1's §10.3.4 fix correctly described the first
   rendered byte as the attribute block (when
   non-empty) or the kind-keyword line, but the
   audit trail in `discussions.md` (D-14-6.4 audit
   narrative, D-14-6.4 worked-example trailer, D-14-6
   cluster opening's D-14-6.2 sub-decision
   allocation) and `prior-art/rustc-render.md` Q3 /
   Q4 still said "first line / first-line keyword
   identifies each Instance" and "output may end with
   a trailing newline produced by the last Instance
   render."
   **Resolution:** Rewrote D-14-6.4's Decision
   paragraph to mirror §10.3.4's corrected wording
   (first rendered byte = attribute block when
   non-empty per §10.2.7, otherwise kind-keyword line;
   ends at last rendered byte `}` or `;` with no
   trailing newline injected; empty case per
   D-14-6.5). Rewrote the D-14-6.4 worked-example
   trailer wording to match. Updated the D-14-6
   cluster opening's D-14-6.2 candidate-(C) rationale
   to describe rank groups as "visually distinct via
   the first rendered line of each `render_instance`
   output -- the attribute block when non-empty per
   §10.2.7, otherwise the kind-keyword line."
   `prior-art/rustc-render.md` Q3 + Q4 recommendation
   paragraphs reworded to match.

3. **[MINOR]** Current status surfaces still
   presented D-14-6 as unopened / queued. The R1
   review-log correctly recorded D-14-6 Phase A as
   complete + Phase B in progress, but
   `stage-14/status.md` and top-level
   `docs/discussions/regseq-model/status.md` still
   had: stage-14 "Locked decisions" entry listing
   D-14-6 as "Queued; entry-steps per README §5.1
   (Step 4-7) NOT yet executed"; "Active cluster"
   saying "None currently in active drafting" with a
   fresh-agent walkthrough on how to open D-14-6;
   "Pre-staged D-tag inventory" Step 6 reading
   "D-14-6 (top-level `render`) -- queued; depends on
   D-14-5"; "Next action" section opening "Open
   D-14-6 cluster" with the Step 4-7 entry plan;
   `(Pending D-14-6 lock.)` stub reference at three
   spots. Top-level `status.md` "Last updated" entry
   still said "Active focus advances to D-14-6".
   **Resolution:** Prepended a fresh "Last updated:
   2026-05-13" entry to both `stage-14/status.md`
   and top-level `status.md` recording D-14-6 Phase A
   LOCKED + Phase B R1+R2 trajectory + R3 close-out
   pending. Updated the stage-14/status.md "Locked
   decisions" / "Active cluster" / "Pre-staged D-tag
   inventory" Step 6 / "Next action" sections to
   reflect the current state. Demoted the fresh-agent
   D-14-6 entry walkthrough (no longer needed; the
   work is done). The `(Pending D-14-6 lock.)` stub
   was R1's spec-text deletion target and has been
   replaced with the §10.3.1-§10.3.7 content; only
   the historical references to the stub (in stage
   navigation prose) remain and were rewritten to
   past-tense.

All 3 R2 findings closed at this pass (0 BLOCK +
3 MINOR + 0 NIT) across
`pretty-printer-snapshot.md` §10.3 / §1.3 / §8
(canonical wording already correct from R1; no
edits) + `discussions.md` D-14-6 cluster opening
D-14-6.2 sub-decision allocation + D-14-6.1
worked-example sketch + D-14-6.3 audit-shape
(Decision + Reasoning + Rejected alternatives +
Methodology rules + Spec impact) + D-14-6.4
audit-shape (Decision + worked-example trailer) +
D-14-6.4 worked-example trailer + `prior-art/rustc-
render.md` Q3 (No-item-header / Implication / (A)/
(B)/(C) candidates / Recommendation) + Q4 (A)
candidate / Recommendation + `stage-14/status.md`
(Last-updated entry prepended + Locked decisions +
Active cluster + Pre-staged D-tag inventory +
Next action section) + top-level `status.md`
(Last-updated entry prepended).

R1 + R2 pattern matches D-14-4 / D-14-5
recap-text-duplication phenomenon (R1 catches
canonical locations; R2 catches the audit-narrative
+ worked-example + recap surfaces). **Next concrete
action**: kick off Phase B R3 close-out Codex review
verifying R2 fixes land cleanly + final close-out
check. On R3 APPROVE / Close-out:YES, the cluster
closes Phase B and Phase A LOCKED status finalises.

### R3 -- 2026-05-13

**Verdict: MINOR / Lockability: yes / Close-out: NO.**

1 finding (0 BLOCK + 1 MINOR + 0 NIT). Cluster
crossed Lockability:yes at R2; R3 confirms the
architectural picture is stable + flags one residual
pre-lock planning-text drift that R1+R2 missed in
non-canonical recap surfaces.

1. **[MINOR]** Residual pre-lock D-14-6 planning text
   remained in two non-canonical recap surfaces. R2's
   canonical fixes landed cleanly at §10.3.3 + §10.3.4
   + §10.3.6 in `pretty-printer-snapshot.md` and at
   the status surfaces in `stage-14/status.md` +
   top-level `status.md`, but two non-canonical recap
   surfaces still described D-14-6 as pre-lock:
   (a) `discussions.md` D-14-6 cluster opening
       Question paragraph said §10.3 was "currently a
       `(Pending D-14-6 lock.)` stub" and that §1.3
       / §8 "will be confirmed / refined at D-14-6.1
       lock", despite the heading already marking
       Phase A LOCKED.
   (b) `memory.md` D-14-6 agenda still presented
       D-14-6 as "next" cluster, retained the
       "Pre-staged sub-decision inventory" framing,
       said the evidence pass "should append" to a
       new `rustc-render.md` file, and kept the
       7-step Phase A drafting plan as forward-
       looking work.
   Both surfaces are stage-internal recap (memory.md
   is not touched by the merger; the discussions.md
   cluster opening is preserved in audit history but
   should still reflect the post-lock state to avoid
   confusing future readers). The drift does not
   change the locked rules.
   **Resolution:**
   - `discussions.md` D-14-6 cluster opening Question
     paragraph rewritten to post-lock state: §10.3
     spec landed at §10.3.1-§10.3.7; §1.3 / §8
     finalised; heading status line updated to
     reflect the Phase B R1 BLOCK -> R2 MINOR/
     Lockability:yes -> R3 close-out pending
     trajectory.
   - `memory.md` D-14-6 agenda rewritten from
     "pre-staged sub-decision inventory" + 7-step
     Phase A drafting plan to a "Locked sub-decisions"
     + "Spec contributions (landed)" + "Step 4 rustc
     evidence landmarks (consumed)" post-lock summary.

All 1 R3 findings closed at this pass (0 BLOCK +
1 MINOR + 0 NIT) across `discussions.md` D-14-6
cluster opening (Question paragraph) + `memory.md`
D-14-6 agenda. **Next concrete action**: kick off
Phase B R4 close-out Codex review verifying R3 fix
lands cleanly + final close-out check. On R4 APPROVE
/ Close-out:YES, the cluster closes Phase B and
finalises as the third and final composer-renderer
cluster of stage 14.

### R4 -- 2026-05-13

**Verdict: MINOR / Lockability: yes / Close-out: NO.**

2 findings (0 BLOCK + 2 MINOR + 0 NIT). Cluster
remains Lockability:yes (architectural picture
stable since R2). R4 caught two close-out polish
items that R1+R2+R3 missed:

1. **[MINOR]** §10.3.7 worked example carried two
   inconsistencies with the locked rules. The
   output-trailer prose said the output ended "with
   the rendering's implicit trailing `\n`",
   contradicting §10.3.4's locked rule that `render`
   does not inject any trailing newline after the
   last Instance and §10.3.3's locked contract that
   `render_instance(id)` outputs end at `}` or `;`
   with no trailing newline. Separately, the
   function-body in the worked-example output text
   omitted the blank line between
   `let mut _0: ();` and `bb0:` that all §9.12
   specimens (130 / 131 / 133) display per the
   §3.1 body envelope (locals-declaration block +
   scope-tree blank-line separator); that
   inconsistency made the §10.3.7 example look like
   `render_instance` produces a different body shape
   than the specimens demonstrate.
   **Resolution:** Rewrote the §10.3.7 output trailer
   to: "ends at the closing `}` byte of
   `struct pl011::Resource`. `render` does not inject
   any trailing newline after the last Instance per
   §10.3.4. The blank line between `let mut _0: ();`
   and `bb0:` is the function-body envelope per §3.1
   (locals-declaration block + scope-tree blank-line
   separator), not a `render`-level separator."
   Added the missing blank line in the function body
   text to match specimens.

2. **[MINOR]** `stage-14/status.md` "Next action"
   section's detailed cluster-state sentence stopped
   the trajectory at R2 + said "R3 close-out
   pending", stale against the top "Last updated"
   entry which records R3 MINOR / Lockability:yes /
   Close-out:NO with 1 finding closed. R3's status
   surface fix correctly updated the top entry but
   missed this duplicate-trajectory sentence further
   down.
   **Resolution:** Rewrote the Next action's
   trajectory sentence to: "R1 BLOCK 6 findings (2
   BLOCK + 4 MINOR; all closed) -> R2 MINOR /
   Lockability:yes / Close-out:NO 3 findings (recap-
   text-duplication cascade; all closed) -> R3 MINOR
   / Lockability:yes / Close-out:NO 1 finding
   (residual pre-lock planning text in `discussions.md`
   D-14-6 cluster opening Question paragraph +
   `memory.md` D-14-6 agenda; both rewritten to
   post-lock state). R4 close-out pending."

All 2 R4 findings closed at this pass (0 BLOCK +
2 MINOR + 0 NIT) across `pretty-printer-snapshot.md`
§10.3.7 (worked-example body + trailer) +
`stage-14/status.md` "Next action" cluster-state
sentence. No stage-13 REOPENs introduced. **Next
concrete action**: kick off Phase B R5 close-out
Codex review verifying R4 fixes land cleanly + final
close-out check. On R5 APPROVE / Close-out:YES, the
cluster closes Phase B.

### R5 -- 2026-05-13

**Verdict: MINOR / Lockability: yes / Close-out: NO.**

2 findings (0 BLOCK + 2 MINOR + 0 NIT). Same
recap-text-duplication cascade: R4's canonical fixes
need to propagate to adjacent surfaces. Architectural
picture stable (Lockability:yes sustained since R2).

1. **[MINOR]** D-14-6.3 and D-14-6.4 audit worked
   examples in `discussions.md` still rendered `bb0:`
   immediately after `let mut _0: ();` without the
   function-body envelope blank line. R4 had fixed
   the canonical §10.3.7 example to match the §9.12
   specimens 130/131/132/133 (which all show the
   blank line per §3.1's body envelope), but the
   adjacent audit worked examples in `discussions.md`
   D-14-6.3 + D-14-6.4 audit-shape entries carried
   the older shape.
   **Resolution:** Inserted the blank line in both
   D-14-6.3 and D-14-6.4 worked-example outputs.

2. **[MINOR]** Three current-state recap surfaces
   still stopped before R4. The stage-14/status.md
   "Last updated" header + top-level status.md
   "Last updated" header + review-log R4 entry all
   correctly recorded R4 MINOR. But three other recap
   surfaces lagged behind:
   (a) `stage-14/status.md` "Next action" detailed
   cluster-state recap still ended with R3 and said
   "R4 close-out pending."
   (b) `discussions.md` D-14-6 heading still said
   "R3 close-out pending."
   (c) `memory.md` D-14-6 overview still said
   "R3 close-out pending."
   **Resolution:** Bumped all three to the R4
   trajectory ("R4 MINOR/Lockability:yes" + "R5
   close-out pending"); detailed `stage-14/status.md`
   Next action recap also gained an R4 trajectory
   summary.

All 2 R5 findings closed at this pass (0 BLOCK +
2 MINOR + 0 NIT) across `discussions.md` D-14-6
heading + D-14-6.3 worked example + D-14-6.4 worked
example + `memory.md` D-14-6 overview +
`stage-14/status.md` Next action cluster-state recap.
No stage-13 REOPENs introduced. **Next concrete
action**: kick off Phase B R6 close-out Codex review
verifying R5 fixes land cleanly + final close-out
check. The cascade is winding down (R3: 1 finding;
R4: 2 findings; R5: 2 findings); R6 anticipated at
0-1 findings APPROVE / Close-out:YES.

### R6 -- 2026-05-13

**Verdict: APPROVE / Lockability: yes / Close-out: YES.**

Zero findings. R5 fixes landed cleanly across all
recap surfaces: D-14-6.3 + D-14-6.4 worked-example
outputs in `discussions.md` carry the function-body
envelope blank line matching §9.12 specimens + R4's
canonical §10.3.7 fix; the D-14-6 heading in
`discussions.md` + `memory.md` D-14-6 overview both
record the R5-pending trajectory; the R5 review-log
entry is present with both findings closed.
Final close-out checks confirm: all 6 D-14-6
sub-decisions remain LOCKED at Phase A;
D-14-6.6 zero-new-stage-13-REOPEN claim holds; §9.12
specimens 129-133 intact; §10.3.1-§10.3.7 canonical
wording internally consistent and consistent with
audit narrative + status surfaces. **The cluster
closes Phase B.**

### Trajectory summary

| Round | Verdict | Lockability | Close-out | Findings (B+M+N) | Notes |
| ----- | ------- | ----------- | --------- | ----------------- | ----- |
| R1    | BLOCK   | no          | NO        | 2 BLOCK + 4 MINOR + 0 NIT = 6 | Load-bearing fixes: separator algorithm re-architected (composer owns `"\n\n"` bytes; resolved without reopening D-14-5); envelope wording corrected (attribute block as first line per §10.2.7, not just keyword line). 4 MINOR: D-tag wording strip + hex Uint -> decimal + codegen-ordering qualification + const-items mis-classified empty-`mir_keys`. |
| R2    | MINOR   | **yes**     | NO        | 0 BLOCK + 3 MINOR + 0 NIT = 3 | Recap-text-duplication cascade: stale separator algorithm in D-14-6.1 sketch + D-14-6.3 audit + prior-art Q3; stale envelope/keyword-line wording in D-14-6.4 audit + D-14-6 cluster opening D-14-6.2 + prior-art Q3/Q4; status surfaces still presenting D-14-6 as queued/pending. |
| R3    | MINOR   | yes         | NO        | 0 BLOCK + 1 MINOR + 0 NIT = 1 | Residual pre-lock D-14-6 planning text in `discussions.md` D-14-6 cluster opening Question paragraph + `memory.md` D-14-6 agenda (still framed D-14-6 as next + Pre-staged inventory + 7-step Phase A drafting plan). |
| R4    | MINOR   | yes         | NO        | 0 BLOCK + 2 MINOR + 0 NIT = 2 | §10.3.7 worked-example output trailer claimed implicit trailing `\n` contradicting §10.3.4 + missing function-body envelope blank line; `stage-14/status.md` Next action trajectory sentence stale at R2/"R3 close-out pending." |
| R5    | MINOR   | yes         | NO        | 0 BLOCK + 2 MINOR + 0 NIT = 2 | D-14-6.3 + D-14-6.4 audit worked examples in `discussions.md` still missing function-body envelope blank line that R4 added to canonical §10.3.7; three recap surfaces lagging R4 trajectory. |
| R6    | **APPROVE** | yes     | **YES**   | 0 BLOCK + 0 MINOR + 0 NIT = 0 | Close-out. R5 fixes landed cleanly; architectural picture stable since R2. |

**Totals across R1-R6**: 14 findings closed (2 BLOCK +
12 MINOR + 0 NIT; 6 rounds). Lockability:yes
sustained across R2-R6 (5 rounds; the entire cascade
was recap-text-duplication, not architectural
reframing). All 6 D-14-6 sub-decisions remained
LOCKED at Phase A unchanged through Phase B revisions
(Phase B revisions were spec-text consistency, not
architectural). **No stage-13 D-tag REOPENs
introduced** by any R1-R5 fix (D-14-6.6 zero-REOPEN
claim holds).

Compared to prior-stage trajectories:

| Stage cluster | Phase B rounds | Total findings |
| ------------- | -------------- | -------------- |
| Stage 14 D-14-1+D-14-3 | 3 | 8 (1B + 4M + 3N) |
| Stage 14 D-14-2 | 6 | 14 (0B + 11M + 3N) |
| Stage 14 D-14-10 | 4 | ~24 (1B + ~17M + ~6N) |
| Stage 14 D-14-4 | 4 | 11 (3B + 4M + 2N + 2 reg-cont) |
| Stage 14 D-14-5 | 4 | 13 (4B + 7M + 2N; 5 R1/R2/R3 continuations + 8 fresh) |
| **Stage 14 D-14-6** | **6** | **14 (2B + 12M + 0N; recap-text-duplication cascade across R2-R5)** |

D-14-6 ran 6 rounds (the longest stage-14 Phase B
trajectory tied with D-14-2's 6 rounds) despite the
cluster having the smallest spec surface
(6 sub-decisions + 5 specimens). The driver was the
unusually long recap-text-duplication tail: the load-
bearing fixes (R1's BLOCKs) propagated to audit
narrative + worked examples + prior-art reasoning
paragraphs + status surface trajectories + cluster
heading + memory.md overview + adjacent audit worked
examples; each round caught the spots the previous
rounds missed. The cascade exhausted at R6. The
architectural picture was stable since R2; the
remaining 5 rounds were spec-text-consistency polish.

**Cluster status**: D-14-6 Phase B LOCKED 2026-05-13
at R6 APPROVE / Lockability:yes / Close-out:YES.
**Cross-stage friction**: none new from D-14-6
(D-14-6.6 zero-REOPEN claim holds across the entire
trajectory). Pending merger-time flips from prior
clusters unchanged: D-9-10 (from D-14-2.5), D-13-6
(from D-14-4.9), plus D-12-1.3 / D-9-9 / D-13-4.3
(from D-14-7 when locked).

---

## Cluster: D-14-7 (`#[naked]` rejection in v1 -- external-review Finding #2)

**Verdict: LOCKED at R6 APPROVE / Lockability:yes /
Close-out:YES (2026-05-13).** Surgical-fix cluster
closing external-review Finding #2; 9 sub-decisions
D-14-7.1 through D-14-7.9 LOCKED at Phase A
(2026-05-13). 6-round Phase B trajectory closed
2026-05-13: R1 BLOCK-free MINOR with 3 findings ->
R2-R5 each MINOR with 1 finding (recap-text-
duplication cascade mirroring D-14-6's 6-round
trajectory exactly) -> R6 APPROVE 0 findings after
the R5 cascade-breaking fix. Total: **7 findings
closed across R1-R5 (0B+7M+0N)**. All 9 D-14-7
sub-decisions remain LOCKED at Phase A unchanged --
Phase B revisions were spec-text consistency
(recap-text-duplication cascade + 3 R1 findings on
field-count / audit-header label / Q5 ALLOW_LIST),
not architectural reframings.

Coverage: `spec-snapshot.md` §3.5.2 + §3.6.6 + §8.0
+ §15.1 edits; `pretty-printer-snapshot.md` §5.1 +
§5.2 + §5.3 + §5.6 + §9.9 edits;
`discussions.md` D-14-7 audit-shape entry (9 sub-
decisions) + §3 status summary updates;
`stage-14/status.md` + top-level `status.md`
current-state updates; `stage-14/memory.md` D-14-7
agenda rewrite; `prior-art/rustc-naked.md` (6 Q-tags
Q1-Q6); `rustc-references.md` "`#[naked]` handling"
section (5 file:line landmarks).

### R1 -- 2026-05-13

**Verdict: MINOR / Lockability: yes / Close-out: NO.**

3 findings (0 BLOCK + 3 MINOR + 0 NIT). Cluster
crossed Lockability:yes at R1 (no BLOCK findings;
architectural picture stable; all findings are
spec-text consistency issues).

1. **[MINOR]** Surviving Item-field count off by one.
   Location: `spec-snapshot.md:2265` field-enumeration
   narrative paragraph + audit recap at
   `discussions.md` D-14-7 Spec-impact section. The
   new narrative said "The 7 Item-level fields above
   (linkage, link_section, inline_hint, is_unsafe + 3
   lang-item flags + opaque_handle_eligible)" but
   that parenthetical enumerates 8 fields, not 7. The
   original D-12-1.3 framing carried 8 typed source-
   level fields (the 7 plus `is_naked`); the right
   trimmed surface is 7 typed source-level fields
   plus the bridge-computed `opaque_handle_eligible`
   outcome (which derives from layout + use-site
   analysis, not from a Rust attribute), keeping the
   typed-source-level vs bridge-computed distinction
   clear.
   **Resolution:** Rewrote §8.0 field-enumeration
   narrative to "The 7 typed source-level metadata
   fields above (linkage, link_section, inline_hint,
   is_unsafe + 3 lang-item flags) plus the bridge-
   computed `opaque_handle_eligible` outcome ... The
   original D-12-1.3 framing carried 8 typed source-
   level fields (the 7 above plus `is_naked`); D-14-7
   trims that surface to 7." The D-14-7 audit recap
   in `discussions.md` updated to mirror the new
   split wording.

2. **[MINOR]** D-14-7 audit impact section header
   used pre-lock drafting label.
   Location: `discussions.md` D-14-7 audit entry
   ("Spec impact preview (Phase A drafting target).").
   The header was carried from the pre-lock draft
   form; at Phase A LOCKED the impact list reflects
   landed edits, not preview targets. Audit-shape
   drift relative to the locked-state status used
   elsewhere in the audit entry.
   **Resolution:** Renamed the header to "Spec
   impact (landed at Phase A lock)."

3. **[MINOR]** `prior-art/rustc-naked.md` Q5
   evidence internally inconsistent: the prose said
   `#[test]` was "Notably absent (incompatible per
   rustc rules)" but the ALLOW_LIST at
   `codegen_attrs.rs:266-269` does list
   `sym::test` (alongside `sym::ignore`,
   `sym::should_panic`, `sym::bench`) -- the
   comment block's incompatibility claim and the
   ALLOW_LIST's admission appear to contradict.
   Location: `prior-art/rustc-naked.md:246` +
   `:260`. Does not affect the locked `#[naked]`
   rejection rule (rustc enforcement already
   handled by the time the bridge observes the
   `CodegenFnAttrFlags::NAKED` bit), but the
   internal inconsistency would surface in any
   future Codex review of the evidence pass.
   **Resolution:** Reworded Q5 prose to clarify
   that `#[inline]` and `#[track_caller]` are the
   substantive incompatibilities (truly absent from
   ALLOW_LIST), while `test` / `ignore` /
   `should_panic` / `bench` appear in ALLOW_LIST
   despite the comment block because rustc admits
   them at the parser level for downstream
   diagnostic-routing (so `rustc_builtin_macros::test`
   can produce its own better-targeted diagnostic),
   not as substantive compatibility statements.

All 3 R1 findings closed at this pass (0 BLOCK +
3 MINOR + 0 NIT) across `spec-snapshot.md` §8.0
field-enumeration narrative + `discussions.md`
D-14-7 audit recap + `discussions.md` D-14-7 audit
impact section header + `prior-art/rustc-naked.md`
Q5 evidence. No new stage-13 / stage-12 / stage-9
REOPENs introduced; the three documented REOPENs
(D-12-1.3, D-9-9, D-13-4.3) per D-14-7.9 unchanged.
**Next concrete action**: kick off Phase B R2
close-out Codex review verifying R1 fixes land
cleanly + final close-out check. On R2 APPROVE /
Close-out:YES, the cluster closes Phase B.

Codex prompt: `/tmp/d147_review/r1_prompt.txt`.
Codex output: `/tmp/d147_review/r1_output.txt`.
Codex lockability rationale (excerpt): findings
classified as MINOR (spec-text consistency) with no
architectural reframings; cluster crossed
Lockability:yes at R1.

### R2 -- 2026-05-13

**Verdict: MINOR / Lockability: yes / Close-out: NO.**

1 finding (0 BLOCK + 1 MINOR + 0 NIT). Cluster
remains Lockability:yes (architectural picture
stable since R1). R2 caught one recap-text-
duplication tail item that R1's status-surface
updates missed -- exactly the pattern from prior
clusters (D-14-4 + D-14-5 + D-14-6 all had
multi-round recap-text-duplication tails).

1. **[MINOR]** Recap surfaces still omit the R1
   trajectory.
   Location: `discussions.md` §3 status summary
   D-14-7 entry tail + `stage-14/memory.md` D-14-7
   cluster-status sentence.
   The canonical R1-trajectory wording landed at
   `stage-14/status.md` "Last updated" header +
   `status.md` "Last updated" header +
   `review-log.md` R1 entry ("Codex Phase B
   trajectory (in progress): R1 MINOR / Lockability:
   yes / Close-out:NO 3 findings ... all closed.
   ... Phase B R2 close-out pending."), but the
   D-14-7 entry in `discussions.md` §3 status
   summary still tailed with "Phase B Codex review
   pending." and the `memory.md` D-14-7 agenda
   "Cluster status" sentence still said "Phase A
   LOCKED 2026-05-13 ...; Phase B Codex review
   pending" rather than acknowledging R1's progress.
   Two recap surfaces lagging behind R1's canonical
   status-header trajectory.
   **Resolution:** Updated the `discussions.md` §3
   D-14-7 entry tail and the `memory.md` D-14-7
   cluster-status sentence to mirror the canonical
   R1-trajectory wording (Codex Phase B in
   progress: R1 MINOR / Lockability:yes / Close-out:
   NO; 3 findings closed; Phase B R2 close-out
   pending).

All 1 R2 findings closed at this pass (0 BLOCK +
1 MINOR + 0 NIT) across `discussions.md` §3 status
summary D-14-7 entry + `memory.md` D-14-7 cluster-
status sentence. No new stage-13 / stage-12 / stage-9
REOPENs introduced; the three documented REOPENs
(D-12-1.3, D-9-9, D-13-4.3) per D-14-7.9 unchanged.
**Next concrete action**: kick off Phase B R3
close-out Codex review verifying R2 fix lands
cleanly + final close-out check. On R3 APPROVE /
Close-out:YES the cluster closes Phase B.

Codex prompt: `/tmp/d147_review/r2_prompt.txt`.
Codex output: `/tmp/d147_review/r2_output.txt`.

### R3 -- 2026-05-13

**Verdict: MINOR / Lockability: yes / Close-out: NO.**

1 finding (0 BLOCK + 1 MINOR + 0 NIT). Cluster
remains Lockability:yes (architectural picture
stable since R1). R3 caught the next-level recap-
text-duplication tail (same pattern as R2, now
shifted one round forward). Four lagging surfaces
all stopped at the R1 trajectory + "Phase B R2
close-out pending" instead of the R2 trajectory +
"Phase B R3 close-out pending."

1. **[MINOR]** R2 trajectory still not mirrored
   across D-14-7 recap surfaces.
   Location: four lagging surfaces:
   (a) `stage-14/discussions.md` §3 status summary
       D-14-7 entry tail still recorded R1
       trajectory + "Phase B R2 close-out pending"
       instead of R2 trajectory + "Phase B R3
       close-out pending."
   (b) `stage-14/discussions.md` D-14-7 cluster
       heading still said "Phase B Codex review
       pending" (heading lagged behind the R1
       trajectory entirely; R2's resolution had
       updated the §3 status entry but missed the
       cluster-heading status line).
   (c) `stage-14/memory.md` D-14-7 cluster-status
       sentence still recorded R1 trajectory +
       "Phase B R2 close-out pending" (R2's
       resolution updated this surface but didn't
       bump it on its own trajectory).
   (d) `stage-14/review-log.md` D-14-7 cluster
       header (above the R1 entry) still said
       "in progress at R1 MINOR / Lockability:yes
       / Close-out:NO" -- R2's resolution added an
       R2 entry but didn't bump the cluster-header
       verdict line.
   All four are the same recap-text-duplication
   class as R2's finding, propagating to surfaces
   R2's fix didn't cover. Canonical R2-trajectory
   wording landed at `stage-14/status.md` "Last
   updated" + top-level `status.md` "Last updated"
   + `review-log.md` R2 entry; the four lagging
   surfaces missed the bump.
   **Resolution:** Updated each of the four lagging
   surfaces to mirror the canonical R2-trajectory
   wording: R1 MINOR ... -> R2 MINOR /
   Lockability:yes / Close-out:NO 1 finding ...
   Phase B R3 close-out pending. The cluster header
   in `review-log.md` flipped from "in progress at
   R1 MINOR ..." to "in progress at R2 MINOR ...";
   the cluster heading in `discussions.md` flipped
   to "Phase B trajectory in progress: R1 MINOR ->
   R2 MINOR/Lockability:yes/Close-out:NO; R3 close-
   out pending."

All 1 R3 findings closed at this pass (0 BLOCK +
1 MINOR + 0 NIT) across `discussions.md` §3
status-summary D-14-7 tail + `discussions.md`
D-14-7 cluster heading + `memory.md` D-14-7
cluster-status sentence + `review-log.md` D-14-7
cluster header. No new stage-13 / stage-12 /
stage-9 REOPENs introduced; the three documented
REOPENs (D-12-1.3, D-9-9, D-13-4.3) per D-14-7.9
unchanged.
**Next concrete action**: kick off Phase B R4
close-out Codex review verifying R3 fixes land
cleanly + final close-out check. On R4 APPROVE /
Close-out:YES the cluster closes Phase B. The
cascade is winding down (R2: 1 finding; R3: 1
finding); R4 anticipated at 0-1 findings.

Codex prompt: `/tmp/d147_review/r3_prompt.txt`.
Codex output: `/tmp/d147_review/r3_output.txt`.

### R4 -- 2026-05-13

**Verdict: MINOR / Lockability: yes / Close-out: NO.**

1 finding (0 BLOCK + 1 MINOR + 0 NIT). Cluster
remains Lockability:yes (architectural picture
stable since R1). R4 caught the same recap-text-
duplication shift one round forward (R2 -> R3 -> R4
each shifted the same surfaces). The four surfaces
R3 bumped to R2-trajectory wording lagged the
canonical R3 trajectory.

1. **[MINOR]** R3 recap fix did not land the R3
   trajectory on the four claimed surfaces.
   Location: canonical surfaces:
   `stage-14/status.md:50`, `status.md:51`,
   `review-log.md:2679` (R3 entry); lagging
   surfaces:
   (a) `stage-14/discussions.md` §3 status summary
       D-14-7 tail stopped at R2 trajectory +
       "Phase B R3 close-out pending."
   (b) `stage-14/discussions.md` D-14-7 cluster
       heading still said "R1 MINOR -> R2 MINOR/
       Lockability:yes/Close-out:NO; R3 close-out
       pending."
   (c) `stage-14/memory.md` D-14-7 cluster-status
       sentence stopped at R2 trajectory.
   (d) `stage-14/review-log.md` D-14-7 cluster
       header verdict line stopped at "in progress
       at R2 MINOR."
   Same recap-text-duplication class as R3, advanced
   one round.
   **Resolution:** Bumped each of the four surfaces
   to canonical R3 wording: R1 MINOR -> R2 MINOR ->
   R3 MINOR/Lockability:yes/Close-out:NO, Phase B
   R4 close-out pending. Cluster heading in
   `discussions.md` flipped to "R1 MINOR -> R2 MINOR
   -> R3 MINOR/Lockability:yes/Close-out:NO; R4
   close-out pending." Cluster header in
   `review-log.md` flipped from "in progress at R2
   MINOR" to "in progress at R3 MINOR." §3 status-
   summary tail + memory.md cluster-status sentence
   each gained an R3 trajectory clause.

All 1 R4 findings closed at this pass (0 BLOCK +
1 MINOR + 0 NIT) across the four lagging surfaces.
No new stage-13 / stage-12 / stage-9 REOPENs
introduced; the three documented REOPENs (D-12-1.3,
D-9-9, D-13-4.3) per D-14-7.9 unchanged.
**Next concrete action**: kick off Phase B R5
close-out Codex review verifying R4 fixes land
cleanly + final close-out check. The cascade
continues (R2: 1 finding; R3: 1 finding; R4: 1
finding); R5 anticipated at 0-1 findings -- same
recap-text-duplication-shift pattern as D-14-6 (R5
caught the same shift one round forward; R6 closed
with zero findings).

Codex prompt: `/tmp/d147_review/r4_prompt.txt`.
Codex output: `/tmp/d147_review/r4_output.txt`.

### R5 -- 2026-05-13

**Verdict: MINOR / Lockability: yes / Close-out: NO.**

1 finding (0 BLOCK + 1 MINOR + 0 NIT). Cluster
remains Lockability:yes (architectural picture
stable since R1). R5 caught the same recap-text-
duplication shift one round forward. The four
surfaces R4 bumped to R3-trajectory wording lagged
the canonical R4 trajectory.

1. **[MINOR]** R4 recap fix did not land R4
   trajectory on the four claimed surfaces.
   Location: canonical surfaces:
   `stage-14/status.md` "Last updated" header +
   top-level `status.md` "Last updated" header +
   `review-log.md` R4 entry; lagging surfaces:
   (a) `stage-14/discussions.md` §3 status summary
       D-14-7 entry tail stopped at R3 trajectory.
   (b) `stage-14/discussions.md` D-14-7 cluster
       heading stopped at "R1 -> R2 -> R3 MINOR;
       R4 close-out pending."
   (c) `stage-14/memory.md` D-14-7 cluster-status
       sentence stopped at R3 trajectory.
   (d) `stage-14/review-log.md` D-14-7 cluster
       header verdict line stopped at "in progress
       at R3 MINOR."
   Same recap-text-duplication-shift class as R3
   and R4, advanced one more round. The cascade is
   structural: each round R's resolution bumps the
   four surfaces to round (R-1)'s trajectory (the
   canonical state at the moment of resolution
   launch), but round R's own resolution + R+1's
   canonical state push the canonical headers one
   round further ahead, leaving the four surfaces
   one round behind.
   **Resolution:** Bumped each of the four lagging
   surfaces to the **full R5 trajectory at once**
   (`R1 -> R2 -> R3 -> R4 -> R5 MINOR ...`) plus
   "Phase B R6 close-out pending" -- including R5
   itself in the trajectory rather than stopping at
   R4. This breaks the one-round-behind cascade by
   matching the canonical R5 trajectory in the same
   pass. The cluster heading in `discussions.md`
   flipped to "Phase B trajectory in progress: R1
   MINOR -> R2 MINOR -> R3 MINOR -> R4 MINOR -> R5
   MINOR/Lockability:yes/Close-out:NO; R6 close-out
   pending." Cluster header in `review-log.md`
   flipped from "in progress at R3 MINOR" to "in
   progress at R5 MINOR."

All 1 R5 findings closed at this pass (0 BLOCK +
1 MINOR + 0 NIT) across the four lagging surfaces.
No new stage-13 / stage-12 / stage-9 REOPENs
introduced; the three documented REOPENs (D-12-1.3,
D-9-9, D-13-4.3) per D-14-7.9 unchanged.
**Next concrete action**: kick off Phase B R6
close-out Codex review verifying R5 fixes land
cleanly + final close-out check. The four lagging
surfaces are now synchronised with the canonical R5
trajectory (the cascade-breaking fix); R6
anticipated at 0 findings APPROVE / Close-out:YES,
mirroring D-14-6's R6 close (same recap-text-
duplication-cascade pattern, same 6-round
trajectory).

Codex prompt: `/tmp/d147_review/r5_prompt.txt`.
Codex output: `/tmp/d147_review/r5_output.txt`.

### R6 -- 2026-05-13

**Verdict: APPROVE / Lockability: yes / Close-out: YES.**

Zero findings. R5's cascade-breaking fix landed
cleanly across all four previously-lagging surfaces:
the canonical R5 trajectory now lives on
`stage-14/discussions.md` §3 status summary D-14-7
tail + `discussions.md` D-14-7 cluster heading +
`stage-14/memory.md` D-14-7 cluster-status sentence
+ `stage-14/review-log.md` D-14-7 cluster header,
matching the `stage-14/status.md` + top-level
`status.md` + R5 review-log entry canonical
surfaces. Final close-out checks confirm: all 9
D-14-7 sub-decisions remain LOCKED at Phase A;
spec edits at `spec-snapshot.md` §3.5.2 + §3.6.6 +
§8.0 + §15.1 and `pretty-printer-snapshot.md` §5.1
+ §5.2 + §5.3 + §5.6 + §9.9 intact and internally
consistent; `prior-art/rustc-naked.md` Q5 reword
stands; `rustc-references.md` "`#[naked]` handling"
section landed (5 file:line landmarks). **The
cluster closes Phase B.**

### Trajectory summary

| Round | Verdict | Lockability | Close-out | Findings (B+M+N) | Notes |
| ----- | ------- | ----------- | --------- | ----------------- | ----- |
| R1    | MINOR   | **yes**     | NO        | 0 BLOCK + 3 MINOR + 0 NIT = 3 | §8.0 field-enumeration narrative "7 Item-level fields" enumerated 8 + D-14-7 audit-section "Spec impact preview" header still pre-lock label + Q5 evidence ALLOW_LIST `test`/`ignore`/`should_panic` admission inconsistency. Cluster crossed Lockability:yes at R1 (no BLOCK findings; architectural picture stable). |
| R2    | MINOR   | yes         | NO        | 0 BLOCK + 1 MINOR + 0 NIT = 1 | Recap-text-duplication tail: §3 status summary D-14-7 tail + `memory.md` cluster-status sentence still said "Phase B Codex review pending" instead of canonical R1-trajectory wording. |
| R3    | MINOR   | yes         | NO        | 0 BLOCK + 1 MINOR + 0 NIT = 1 | Next-level recap-text-duplication tail across four lagging surfaces (§3 + cluster heading + memory.md cluster-status + review-log cluster header) all stopped at R1 trajectory. |
| R4    | MINOR   | yes         | NO        | 0 BLOCK + 1 MINOR + 0 NIT = 1 | Same four surfaces shifted one round forward: R3 bumped to R2-trajectory wording but lagged canonical R3 trajectory. |
| R5    | MINOR   | yes         | NO        | 0 BLOCK + 1 MINOR + 0 NIT = 1 | Same shift forward: R4 bumped to R3-trajectory wording but lagged canonical R4 trajectory. **Cascade-breaking fix applied: four surfaces bumped to FULL R5 trajectory at once including R5 itself + "Phase B R6 close-out pending."** |
| R6    | **APPROVE** | yes     | **YES**   | 0 BLOCK + 0 MINOR + 0 NIT = 0 | Close-out. R5 cascade-breaking fix landed cleanly; canonical R5 trajectory matches all four previously-lagging surfaces. |

**Totals across R1-R6**: 7 findings closed (0 BLOCK +
7 MINOR + 0 NIT; 6 rounds). Lockability:yes
sustained across R1-R6 (the entire cascade was
recap-text-duplication, not architectural
reframing; the 3 R1 findings were spec-text
consistency too). All 9 D-14-7 sub-decisions
remained LOCKED at Phase A unchanged through Phase
B revisions. **No new stage-13 D-tag REOPENs
introduced** by any R1-R5 fix; the three documented
REOPENs (D-12-1.3, D-9-9, D-13-4.3) per D-14-7.9
unchanged.

Compared to prior-stage trajectories:

| Stage cluster | Phase B rounds | Total findings |
| ------------- | -------------- | -------------- |
| Stage 14 D-14-1+D-14-3 | 3 | 8 (1B + 4M + 3N) |
| Stage 14 D-14-2 | 6 | 14 (0B + 11M + 3N) |
| Stage 14 D-14-10 | 4 | ~24 (1B + ~17M + ~6N) |
| Stage 14 D-14-4 | 4 | 11 (3B + 4M + 2N + 2 reg-cont) |
| Stage 14 D-14-5 | 4 | 13 (4B + 7M + 2N; 5 R1/R2/R3 continuations + 8 fresh) |
| Stage 14 D-14-6 | 6 | 14 (2B + 12M + 0N; recap-text-duplication cascade across R2-R5) |
| **Stage 14 D-14-7** | **6** | **7 (0B + 7M + 0N; recap-text-duplication cascade across R2-R5 + cascade-breaking fix at R5)** |

D-14-7 ran 6 rounds (matching D-14-2 + D-14-6's 6-round
trajectories) despite the cluster having the smallest
spec surface so far (9 sub-decisions, ~10 distinct edit
locations, no specimens added). The driver was the
same recap-text-duplication-cascade pattern that hit
D-14-6: each round's surface-bump landed at the
previous round's trajectory wording, leaving the four
surfaces one round behind canonical. The R5 cascade-
breaking fix (bumping the four surfaces to the full
R5 trajectory **at once** including R5 itself) ended
the cascade in one pass; R6 confirmed APPROVE.

**Cluster status**: D-14-7 Phase B LOCKED 2026-05-13
at R6 APPROVE / Lockability:yes / Close-out:YES.
**Cross-stage friction**: three new REOPENs from
D-14-7 (D-12-1.3, D-9-9, D-13-4.3) join the pending
merger-time flips from prior clusters (D-9-10 from
D-14-2.5, D-13-6 from D-14-4.9). The five-decision
merger-time flip queue (D-9-10, D-13-6, D-12-1.3,
D-9-9, D-13-4.3) will land at stage-14 merger close
per §5.5 step 4. **Phase 1 spec closure**: D-14-7
closes the first surgical-fix cluster (external-
review Finding #2). The remaining stage-14 work is
two more surgical-fix clusters (D-14-8 stale-
forward-pointer retarget per Finding #3; D-14-9
Specimen 70 split per Finding #4) and
`core::ffi::c_void` rejection enforcement; after
those clusters lock, the per-stage cross-cluster
Codex review on the full `spec-snapshot.md` +
`pretty-printer-snapshot.md` runs (anticipated 2-4
rounds), then hand-off to merger per §5.5 7-step
contract.

Codex prompt: `/tmp/d147_review/r6_prompt.txt`.
Codex output: `/tmp/d147_review/r6_output.txt`.

---

## Cluster: D-14-8 (stale `emission_name` forward-pointer retarget -- external-review Finding #3)

**Verdict: LOCKED at R2 APPROVE / Lockability:yes /
Close-out:YES (2026-05-13).** Pure spec hygiene
cluster closing external-review Finding #3; no
sub-decisions; no D-tag flip. Phase A LOCKED
2026-05-13 with a single applied edit at
`spec-snapshot.md` §9.2 paragraph 3 (the
`Struct / Enum / Union` `emission_name` bridge-
construction-rule bullet, closing clause). 2-round
Phase B trajectory closed 2026-05-13: R1 BLOCK-free
MINOR with 2 findings (tracker line-number wording
+ placeholder D-9-? in audit cross-stage friction)
-> R2 APPROVE 0 findings. Total: **2 findings
closed across R1 (0B+2M+0N)**. Both findings were
spec-text consistency at the audit + tracker
surfaces, not architectural reframings. **No
cascade pattern** -- the single-paragraph edit had
no recap surfaces to drift; the 2-round trajectory
matches what the trackers anticipated at Phase A
lock ("R1 anticipated at 0-1 findings APPROVE /
Close-out:YES").

Coverage: `spec-snapshot.md` §9.2 paragraph 3 edit
(7-line surgical replacement of the
`Struct / Enum / Union` `emission_name`
construction-rule bullet closing clause);
`discussions.md` D-14-8 audit-shape entry (no
sub-decisions, ~220-line section appended after
D-14-7); `discussions.md` preamble cluster list +
§3 status summary D-14-8 entry; `stage-14/status.md`
Lead + Next-action sections updated; top-level
`status.md` Lead updated; `stage-14/memory.md`
D-14-8 section bumped from one-line stub to closed-
state recap. **No fresh rustc evidence pass
required** for this cluster -- the directive is
prescriptive, the `tcx.symbol_name` codepath used
by the callable arms is unchanged, and the
type-instance bridge-construction algorithm has no
rustc precedent because rustc has no equivalent
artifact.

### R1 -- 2026-05-13

**Verdict: MINOR / Lockability:yes / Close-out:NO.**

2 findings (0 BLOCK + 2 MINOR + 0 NIT). Cluster
crossed Lockability:yes at R1 (no BLOCK findings;
architectural picture stable; both findings are
spec-text consistency issues).

1. **[MINOR]** Tracker leads imply the stale
   forward-pointer text is still at line 2452.
   Location: `discussions.md` §3 status summary
   D-14-8 entry + `stage-14/status.md` Lead +
   top-level `status.md` Lead. Each tracker said
   "the stale 'stage 13 (codegen)' forward-pointer
   at `spec-snapshot.md` §9.2 paragraph 3 / line
   2452" -- after the Phase A edit, line 2452 now
   carries the retarget text, so the phrasing
   incorrectly implied the stale text was still
   present at the cited line. The directive itself
   (Finding #3 at `expectations.md`) is correctly
   matched by the spec edit; this was a tracker
   wording issue, not a spec issue.
   **Resolution:** Reworded the three tracker
   leads to "the formerly stale 'stage 13
   (codegen)' forward-pointer at ... §9.2
   paragraph 3; current retarget text begins at
   line 2452 after the Phase A edit." Past-tense
   framing + explicit acknowledgement that the
   line now carries the corrected text.

2. **[MINOR]** Audit entry's cross-stage friction
   section carried a placeholder `D-9-?` for the
   stage-9 `emission_name` field shape claim.
   Location: `discussions.md` D-14-8 cross-stage
   friction section. The text "Stage 9 D-9-?
   `emission_name` field shape" used an
   unresolved D-tag placeholder; the stage-9
   decision registry has no concrete D-9 tag for
   the `emission_name` field shape (the field
   sits in the §9.2 Instance schema authored at
   stage 9 without a per-field D-tag), and a
   locked audit trail should not carry placeholder
   tags.
   **Resolution:** Replaced "Stage 9 D-9-?
   `emission_name` field shape" with "Stage-9
   inherited §9.2 `emission_name` text"; added
   explicit citations to F4-2 (canonical instance-
   interning-key ordering at §9.3) and D-12-2.4
   (the link_name -> emission_name fold for the
   callable arms) to ground the claim that
   prior-stage decisions are unchanged.

### R2 -- 2026-05-13

**Verdict: APPROVE / Lockability:yes / Close-out:YES.**

0 findings. R1's two MINOR findings both verified
closed by Codex against the file state:
- R1-#1 (tracker wording): verified closed at
  `discussions.md` §3 status summary D-14-8 entry
  + `stage-14/status.md` Lead + top-level
  `status.md` Lead; all three now read "formerly
  stale ... current retarget text begins at line
  2452 after the Phase A edit."
- R1-#2 (D-9-? placeholder): verified closed at
  `discussions.md` D-14-8 cross-stage friction
  section; no `D-9-?` hits remain; F4-2 and
  D-12-2.4 citations land.

No new findings. Cluster closes Phase B.

### Trajectory summary

| Round | Verdict | Lockability | Close-out | Findings (B+M+N) | Notes |
| ----- | ------- | ----------- | --------- | ----------------- | ----- |
| R1    | MINOR   | **yes**     | NO        | 0 BLOCK + 2 MINOR + 0 NIT = 2 | Tracker line-number wording at three surfaces + placeholder `D-9-?` in audit cross-stage friction section. Cluster crossed Lockability:yes at R1 (no BLOCK findings; architectural picture stable). |
| R2    | **APPROVE** | yes     | **YES**   | 0 BLOCK + 0 MINOR + 0 NIT = 0 | Close-out. Both R1 fixes verified closed by Codex against the file state; no new findings. |

**Totals across R1-R2**: 2 findings closed (0 BLOCK
+ 2 MINOR + 0 NIT; 2 rounds). Lockability:yes
sustained across R1-R2 (no BLOCK findings; both
findings spec-text consistency at audit + tracker
surfaces). **No new D-tag REOPENs introduced** by
any R1 fix; the five-decision merger-time flip queue
(D-9-10, D-13-6, D-12-1.3, D-9-9, D-13-4.3) is
unchanged.

Compared to prior-stage trajectories:

| Stage cluster | Phase B rounds | Total findings |
| ------------- | -------------- | -------------- |
| Stage 14 D-14-1+D-14-3 | 3 | 8 (1B + 4M + 3N) |
| Stage 14 D-14-2 | 6 | 14 (0B + 11M + 3N) |
| Stage 14 D-14-10 | 4 | ~24 (1B + ~17M + ~6N) |
| Stage 14 D-14-4 | 4 | 11 (3B + 4M + 2N + 2 reg-cont) |
| Stage 14 D-14-5 | 4 | 13 (4B + 7M + 2N; 5 R1/R2/R3 continuations + 8 fresh) |
| Stage 14 D-14-6 | 6 | 14 (2B + 12M + 0N; recap-text-duplication cascade across R2-R5) |
| Stage 14 D-14-7 | 6 | 7 (0B + 7M + 0N; recap-text-duplication cascade across R2-R5 + cascade-breaking fix at R5) |
| **Stage 14 D-14-8** | **2** | **2 (0B + 2M + 0N; smallest cluster -- pure hygiene; no cascade)** |

D-14-8 ran 2 rounds -- the shortest Phase B
trajectory in stage 14 so far. The driver is the
cluster's scope: a single-paragraph spec edit fully
prescribed by Finding #3, no sub-decisions, no
architectural choice, no specimens, no
recap-text-duplication surfaces. The two R1
findings landed at audit + tracker surfaces only;
R2 confirmed APPROVE after surgical wording fixes.
This matches the Phase A trackers' anticipation
("R1 anticipated at 0-1 findings APPROVE /
Close-out:YES; no cascade pattern expected unlike
D-14-6 / D-14-7 where multi-cluster recap surfaces
drove the 6-round trajectories"); the only slip
was one extra MINOR over the anticipated 0-1, but
the close-out closed at R2 rather than R1 as a
result.

**Cluster status**: D-14-8 Phase B LOCKED 2026-05-13
at R2 APPROVE / Lockability:yes / Close-out:YES.
**Cross-stage friction**: none new from D-14-8. The
five-decision merger-time flip queue (D-9-10,
D-13-6, D-12-1.3, D-9-9, D-13-4.3) is unchanged.
**Phase 1 spec closure**: D-14-8 closes the second
surgical-fix cluster (external-review Finding #3).
Remaining stage-14 work is **D-14-9** (Specimen 70
split per Finding #4) + **`core::ffi::c_void`
rejection enforcement** + per-stage cross-cluster
Codex review on the full `spec-snapshot.md` +
`pretty-printer-snapshot.md` (anticipated 2-4
rounds), then hand-off to merger per §5.5 7-step
contract.

Codex artifacts: prompts and outputs at
`/tmp/codex_d14_8_r1.txt` (R1) and
`/tmp/codex_d14_8_r2.txt` (R2).

---

## Cluster: D-14-9 (Specimen 70 split -- external-review Finding #4)

**Verdict: LOCKED at R2 APPROVE / Lockability:yes /
Close-out:YES (2026-05-13).** Pure pretty-printer-
spec hygiene cluster closing external-review
Finding #4; no sub-decisions; no D-tag flip on
D-13-7. Phase A LOCKED 2026-05-13 with two applied
specimen edits at `pretty-printer-snapshot.md`
§9.9 (Specimen 70 downshift + Specimen 70b
addition) and removal of the D-14-7 cross-finding
parenthetical. 2-round Phase B trajectory closed
2026-05-13: R1 BLOCK-free MINOR with 2 findings
(stale "three clusters" preamble wording + audit
overstatement of how Specimen 70b differs from
Specimen 73) -> R2 APPROVE 0 findings. Total: **2
findings closed across R1 (0B+1M+1N)**. Both
findings were spec-text consistency at the audit +
preamble surfaces, not architectural reframings.
**No cascade pattern** -- the two specimen edits
had no recap surfaces to drift; matches D-14-8's
shortest-trajectory shape.

Coverage: `pretty-printer-snapshot.md` §9.9
Specimen 70 input downshift (`opaque_handle_eligible:
true` -> `false`) + output line removal
(`#[opaque_handle]` dropped) + D-14-7 cross-finding
parenthetical removal; new Specimen 70b on
`ItemKind::Struct` with `def_path: <pl011::Handle>`
and `opaque_handle_eligible: true` rendering
`#[opaque_handle]` above `struct pl011::Handle`;
`discussions.md` D-14-9 audit-shape entry (no
sub-decisions, ~260-line section appended after
D-14-8); `discussions.md` preamble cluster list +
§3 status summary D-14-9 entry; `stage-14/status.md`
Lead + Next-action sections updated; top-level
`status.md` Lead updated; `stage-14/memory.md`
D-14-9 section bumped from one-line stub to
closed-state recap. **No fresh rustc evidence pass
required** for this cluster -- specimen content
hygiene only; the opaque-handle eligibility
constraint at `spec-snapshot.md` §8.0 invariant 19
is already locked.

### R1 -- 2026-05-13

**Verdict: MINOR / Lockability:yes / Close-out:NO.**

2 findings (0 BLOCK + 1 MINOR + 1 NIT). Cluster
crossed Lockability:yes at R1 (no BLOCK findings;
architectural picture stable; both findings are
spec-text consistency at the audit + preamble
layer).

1. **[MINOR]** Stale "three clusters" wording in
   preamble cross-cluster-handoff sentence.
   Location: `discussions.md:111` preamble
   "Remaining stage-14 work" handoff sentence.
   After D-14-9 moved from "Remaining" to "Closed"
   in the preamble cluster list, the remaining
   work is just one cluster (`core::ffi::c_void`
   rejection enforcement), but the handoff
   sentence still said "After those three clusters
   lock" -- stale by two clusters (D-14-7 +
   D-14-8 had already closed before D-14-9
   opened).
   **Resolution:** Reworded to "After that
   remaining cluster locks, the per-stage cross-
   cluster Codex review on the full
   `spec-snapshot.md` + `pretty-printer-snapshot.md`
   runs (anticipated 2-4 rounds), then hand-off
   to merger per `../README.md` §5.5 7-step
   contract."

2. **[NIT]** Audit entry overstated how Specimen
   70b differs from Specimen 73.
   Location: `discussions.md` D-14-9 prior-art
   evidence paragraph (~line 10234). The text
   "mirrors Specimen 73 (the baseline Struct
   specimen at §9.9), differing only in the
   `opaque_handle_eligible: true` flag and the
   resulting `#[opaque_handle]` rendered line"
   was too strong -- Specimen 70b uses
   `def_path: <pl011::Handle>` while Specimen 73
   uses `def_path: <pl011::PL011>`, so the
   `def_path` also differs.
   **Resolution:** Reworded to "mirrors the
   baseline Struct rendering shape from Specimen
   73 at §9.9, while using `pl011::Handle`
   (Specimen 73 uses `pl011::PL011`),
   `opaque_handle_eligible: true`, and the
   resulting `#[opaque_handle]` rendered line."

### R2 -- 2026-05-13

**Verdict: APPROVE / Lockability:yes / Close-out:YES.**

0 findings. R1's two findings both verified closed
by Codex against the file state:
- R1-#1 (stale "three clusters" wording): verified
  closed at `discussions.md:111` -- the preamble
  now reads "After that remaining cluster locks"
  before the cross-cluster review handoff.
- R1-#2 (Specimen 70b vs 73 overstatement):
  verified closed at `discussions.md:10234`-10238 --
  the audit now says Specimen 70b mirrors the
  baseline Struct rendering shape while using
  `pl011::Handle`, and explicitly notes Specimen
  73 uses `pl011::PL011`.

No new findings. Cluster closes Phase B.

### Trajectory summary

| Round | Verdict | Lockability | Close-out | Findings (B+M+N) | Notes |
| ----- | ------- | ----------- | --------- | ----------------- | ----- |
| R1    | MINOR   | **yes**     | NO        | 0 BLOCK + 1 MINOR + 1 NIT = 2 | Stale "three clusters" preamble wording + audit overstatement of Specimen 70b vs 73 differences. Cluster crossed Lockability:yes at R1 (no BLOCK findings; architectural picture stable). |
| R2    | **APPROVE** | yes     | **YES**   | 0 BLOCK + 0 MINOR + 0 NIT = 0 | Close-out. Both R1 fixes verified closed; no new findings. |

**Totals across R1-R2**: 2 findings closed (0 BLOCK
+ 1 MINOR + 1 NIT; 2 rounds). Lockability:yes
sustained across R1-R2 (no BLOCK findings; both
findings spec-text consistency at audit + preamble
surfaces). **No new D-tag REOPENs introduced** by
any R1 fix; the five-decision merger-time flip queue
(D-9-10, D-13-6, D-12-1.3, D-9-9, D-13-4.3) is
unchanged.

Compared to prior-stage trajectories:

| Stage cluster | Phase B rounds | Total findings |
| ------------- | -------------- | -------------- |
| Stage 14 D-14-1+D-14-3 | 3 | 8 (1B + 4M + 3N) |
| Stage 14 D-14-2 | 6 | 14 (0B + 11M + 3N) |
| Stage 14 D-14-10 | 4 | ~24 (1B + ~17M + ~6N) |
| Stage 14 D-14-4 | 4 | 11 (3B + 4M + 2N + 2 reg-cont) |
| Stage 14 D-14-5 | 4 | 13 (4B + 7M + 2N; 5 R1/R2/R3 continuations + 8 fresh) |
| Stage 14 D-14-6 | 6 | 14 (2B + 12M + 0N; recap-text-duplication cascade across R2-R5) |
| Stage 14 D-14-7 | 6 | 7 (0B + 7M + 0N; recap-text-duplication cascade across R2-R5 + cascade-breaking fix at R5) |
| Stage 14 D-14-8 | 2 | 2 (0B + 2M + 0N; smallest cluster -- pure hygiene; no cascade) |
| **Stage 14 D-14-9** | **2** | **2 (0B + 1M + 1N; pure pretty-printer hygiene; no cascade)** |

D-14-9 ran 2 rounds -- tied with D-14-8 for the
shortest Phase B trajectory in stage 14. The two
small surgical-fix clusters (D-14-8 + D-14-9) both
closed in 2 rounds at 2 findings each, vs the 6-
round / 7-finding D-14-7 surgical-fix cluster
(which had nine sub-decisions + ten distinct edit
locations + the recap-text-duplication cascade).
The cleaner shape of D-14-8 / D-14-9 (no sub-
decisions, no specimen-renumbering ripple, no
recap surfaces beyond the standard tracker
quartet) is the dominant driver of the shorter
trajectories.

**Cluster status**: D-14-9 Phase B LOCKED 2026-05-13
at R2 APPROVE / Lockability:yes / Close-out:YES.
**Cross-stage friction**: none new from D-14-9. The
five-decision merger-time flip queue (D-9-10,
D-13-6, D-12-1.3, D-9-9, D-13-4.3) is unchanged.
**Phase 1 spec closure**: D-14-9 closes the third
surgical-fix cluster (external-review Finding #4).
Remaining stage-14 work is **`core::ffi::c_void`
rejection enforcement** + per-stage cross-cluster
Codex review on the full `spec-snapshot.md` +
`pretty-printer-snapshot.md` (anticipated 2-4
rounds), then hand-off to merger per §5.5 7-step
contract.

Codex artifacts: prompts and outputs at
`/tmp/codex_d14_9_r1.txt` (R1) and
`/tmp/codex_d14_9_r2.txt` (R2).

---

## Cluster: `core::ffi::c_void` rejection enforcement (no D-tag; hygiene cluster)

**Verdict: LOCKED at R3 APPROVE / Lockability:yes /
Close-out:YES (2026-05-13).** Pure spec hygiene
cluster closing the inherited stage-3 D16 + D1.4
expectation. No D-tag allocated (stage-3 D16 already
covers the decision-level commitment); no D-tag
flip; no sub-decisions. Phase A LOCKED 2026-05-13
with a single applied edit at
`spec-snapshot.md` §15.1 introduction (two new
paragraphs: Enforcement model + Opaque-struct
pattern definition + Appendix A.1 forward-
reference). 3-round Phase B trajectory closed
2026-05-13: R1 BLOCK-free MINOR with 3 findings
(Coroutine family double-classification + stale
line numbers + dangling "Phase B." fragment) ->
R2 BLOCK-free MINOR with 1 finding (missed stale
line number at `memory.md` -- same class as
R1-#2 but at a surface not touched in the R1 fix
sweep) -> R3 APPROVE 0 findings. Total: **4
findings closed across R1-R2 (0B+3M+1N; 3
rounds)**. All findings were spec-text consistency
at the audit + tracker + spec-introduction text
surfaces; the structural directive (two new §15.1
introduction paragraphs naming the §15.0 gate +
defining the opaque-struct pattern) is intact
through Phase B.

Coverage: `spec-snapshot.md` §15.1 introduction
(~28 new lines: Enforcement-model paragraph
naming the §15.0 dispatch-ordering gate as the
algorithmic enforcement for special-ADT "type
translation"-phase rows, enumerating which §15.1
rows the gate intercepts, noting non-ADT type-
translation + Item-attribute + `PlaceElem` rows
are enforced at their owning sections' check
points; Opaque-struct-pattern paragraph defining
the pattern inline + admittance path via
`Ty::Struct` + downstream C-ABI lowering per §14.5
+ §8.0 invariant 19 + forward-reference to
Appendix A.1's end-to-end rewrite); `discussions.md`
cluster audit-shape entry (no D-tag; no sub-
decisions; ~280-line section appended after
D-14-9); `discussions.md` preamble cluster list +
§3 status summary entry; `stage-14/status.md` Lead +
Next-action sections updated; top-level `status.md`
Lead updated; `stage-14/memory.md` cluster section
bumped from one-line stub to closed-state recap.
**No fresh rustc evidence pass required** -- the
recognition mechanism + structural shape of the
opaque-struct pattern are already specified at
§15.0 + §15.1 + Appendix A.1.

### R1 -- 2026-05-13

**Verdict: MINOR / Lockability:yes / Close-out:NO.**

3 findings (0 BLOCK + 2 MINOR + 1 NIT). Cluster
crossed Lockability:yes at R1 (no BLOCK findings;
architectural picture stable; all findings are
spec-text consistency at the spec-introduction +
audit + tracker layers).

1. **[MINOR]** Coroutine family double-classified
   in the new §15.1 enforcement paragraph.
   Location: `spec-snapshot.md` §15.1
   "Enforcement model" paragraph. The new
   paragraph listed `Coroutine` family both under
   the §15.0 special-ADT dispatch gate (gate-
   target enumeration) and under the non-ADT
   examples list ("Coroutine family at TyKind
   level"), making the enforcement checkpoint
   ambiguous: it could not be both.
   **Resolution:** Removed `Coroutine` family
   from the non-ADT examples list. Coroutine
   family now appears only in the §15.0 gate-
   target enumeration, consistent with §15.0's
   own framing (which groups Coroutine family
   under "DSL-rejected ADTs" alongside Box,
   c_void, Atomic*, Pat, UnsafeBinder, Foreign --
   technically Coroutine variants are non-Adt
   TyKinds in rustc, but §15.0's framing groups
   them with the gate-target set; the
   enforcement paragraph honors §15.0's framing).

2. **[MINOR]** Stale line-number citations at six
   audit + tracker sites after the spec edit
   pushed all later lines down.
   Location: `discussions.md` §3 status summary
   D-14-8/c_void entry + `discussions.md` cluster
   audit-shape entry (Evidence-basis bullets 1,
   2, 3, 4) + `stage-14/status.md` Lead +
   top-level `status.md` Lead. After the new
   §15.1 introduction paragraphs landed, the
   c_void row's line shifted from 5899 to 5933;
   `Foreign(_)` from 5894 to 5928; §15.0
   dispatch order from 5851-5874 to a shifted
   range; Appendix A.1 from 12959-12986 to
   12993-13025; etc. Six sites cited the old
   line numbers.
   **Resolution:** Replaced specific line
   references with section anchors (e.g., "the
   §15.1 rejection table"; "§15.0's 'Special-ADT
   dispatch order' subsection"; "Appendix A.1
   walks the rewrite"). Section anchors are
   line-shift-robust.

3. **[NIT]** Dangling "Phase B." fragment at the
   audit closure check.
   Location: `discussions.md` cluster audit-
   shape entry, immediately after the closure
   check paragraph. A standalone "Phase B."
   line trailed the closure check, breaking the
   audit-entry shape.
   **Resolution:** Deleted the stray line. The
   audit entry now ends at the closure check
   paragraph.

### R2 -- 2026-05-13

**Verdict: MINOR / Lockability:yes / Close-out:NO.**

1 finding (0 BLOCK + 1 MINOR + 0 NIT). R1's three
findings all verified closed by Codex; one missed-
surface finding surfaced:

1. **[MINOR]** Missed stale line number at
   `stage-14/memory.md` -- same class as R1-#2
   but at a surface not touched in the R1 fix
   sweep.
   Location: `stage-14/memory.md`
   `core::ffi::c_void` rejection enforcement
   section. The text "the §15.1 c_void row
   already exists (line 5899; ...)" carried the
   stale line number.
   **Resolution:** Replaced with "the §15.1
   c_void row already exists in the rejection
   table (`core::ffi::c_void` | type
   translation | ...)" -- same section-anchor
   pattern as the R1-#2 fixes elsewhere.

### R3 -- 2026-05-13

**Verdict: APPROVE / Lockability:yes / Close-out:YES.**

0 findings. R2's single finding verified closed by
Codex at `stage-14/memory.md`. Codex confirmed the
spec edits, audit entry, preamble + §3 status
summary, and all four trackers
(`stage-14/memory.md` + `stage-14/status.md` +
top-level `status.md` + the discussions.md surfaces)
all carry the canonical post-fix state with no
stale line numbers remaining anywhere.

No new findings. Cluster closes Phase B.

### Trajectory summary

| Round | Verdict | Lockability | Close-out | Findings (B+M+N) | Notes |
| ----- | ------- | ----------- | --------- | ----------------- | ----- |
| R1    | MINOR   | **yes**     | NO        | 0 BLOCK + 2 MINOR + 1 NIT = 3 | Coroutine family double-classified in the new §15.1 enforcement paragraph + stale line-number citations at six audit + tracker sites + dangling "Phase B." fragment. Cluster crossed Lockability:yes at R1 (no BLOCK findings). |
| R2    | MINOR   | yes         | NO        | 0 BLOCK + 1 MINOR + 0 NIT = 1 | Missed stale line number at `memory.md` -- same class as R1-#2 but at a surface not touched in the R1 fix sweep. |
| R3    | **APPROVE** | yes     | **YES**   | 0 BLOCK + 0 MINOR + 0 NIT = 0 | Close-out. R2's single finding verified closed; no new findings. |

**Totals across R1-R2**: 4 findings closed (0 BLOCK
+ 3 MINOR + 1 NIT; 3 rounds). Lockability:yes
sustained across R1-R3 (no BLOCK findings; all
findings spec-text consistency at audit + tracker
+ spec-introduction surfaces). **No new D-tag
REOPENs introduced** by any R1-R2 fix; the five-
decision merger-time flip queue (D-9-10, D-13-6,
D-12-1.3, D-9-9, D-13-4.3) is unchanged.

Compared to prior-stage trajectories:

| Stage cluster | Phase B rounds | Total findings |
| ------------- | -------------- | -------------- |
| Stage 14 D-14-1+D-14-3 | 3 | 8 (1B + 4M + 3N) |
| Stage 14 D-14-2 | 6 | 14 (0B + 11M + 3N) |
| Stage 14 D-14-10 | 4 | ~24 (1B + ~17M + ~6N) |
| Stage 14 D-14-4 | 4 | 11 (3B + 4M + 2N + 2 reg-cont) |
| Stage 14 D-14-5 | 4 | 13 (4B + 7M + 2N; 5 R1/R2/R3 continuations + 8 fresh) |
| Stage 14 D-14-6 | 6 | 14 (2B + 12M + 0N; recap-text-duplication cascade across R2-R5) |
| Stage 14 D-14-7 | 6 | 7 (0B + 7M + 0N; recap-text-duplication cascade across R2-R5 + cascade-breaking fix at R5) |
| Stage 14 D-14-8 | 2 | 2 (0B + 2M + 0N; smallest cluster -- pure hygiene; no cascade) |
| Stage 14 D-14-9 | 2 | 2 (0B + 1M + 1N; pure pretty-printer hygiene; no cascade) |
| **c_void enforcement (no D-tag)** | **3** | **4 (0B + 3M + 1N; pure §15.1-introduction hygiene; one extra round driven by missed-surface stale line number not caught in R1 fix sweep)** |

The c_void cluster ran 3 rounds instead of 2 (the
D-14-8 / D-14-9 baseline for pure-hygiene clusters)
because the R1 fix sweep missed one of the seven
surfaces carrying stale line numbers. Lesson for
future hygiene clusters with widely-distributed
tracker surfaces: enumerate **every** surface where
the stale reference appears at the start of the fix
sweep (grep-driven inventory) rather than fixing
surfaces piecemeal as they surface in Codex
findings. Both `stage-14/memory.md` carried the
stale "line 5899" reference and was missed because
the R1 fix sweep focused on `discussions.md` +
the two `status.md` files first.

**Cluster status**: c_void enforcement Phase B
LOCKED 2026-05-13 at R3 APPROVE / Lockability:yes
/ Close-out:YES. **Cross-stage friction**: none new
from this cluster. The five-decision merger-time
flip queue (D-9-10, D-13-6, D-12-1.3, D-9-9,
D-13-4.3) is unchanged. **Phase 1 spec closure**:
this cluster closes the **fourth and final stage-14
spec surface** (the inherited stage-3 D16 + D1.4
expectation). **All stage-14 clusters are now
LOCKED at Phase B**. Next: per-stage cross-cluster
Codex review on the full `spec-snapshot.md` +
`pretty-printer-snapshot.md` per README §5.5 step 6
+ §5.6 Tier-2 discipline (anticipated 2-4 rounds),
then hand-off to merger per §5.5 7-step contract.

Codex artifacts: prompts and outputs at
`/tmp/codex_cvoid_r1.txt` (R1),
`/tmp/codex_cvoid_r2.txt` (R2),
`/tmp/codex_cvoid_r3.txt` (R3).

---

## Per-stage cross-cluster review (Tier-2 per README §5.5 step 6 / §5.6)

**Verdict: APPROVE / Lockability:yes / Close-out:YES
(R8, 2026-05-13).** Per README §5.5 step 6 + §5.6
Tier-2 discipline, the cross-cluster review pass on
the integrated stage-14 spec contribution catches
inter-cluster drift across the full stage-14 spec
surface before hand-off to the merger. All eleven
stage-14 cluster surfaces had individually closed
Phase B at APPROVE; this review pass validated their
integrated state. 8-round trajectory closed
2026-05-13: R1 MINOR 5 findings -> R2 MINOR 1
finding -> R3 MINOR 1 finding (deeper-scope fix --
narrowed §15.0 + §15.1 special-ADT framing to
actual ADT TyKinds, moved non-Adt TyKinds to
top-level dispatch enforcement) -> R4 MINOR 2
findings -> R5 MINOR 2 findings (recap-layer drift)
-> R6 MINOR 1 finding -> R7 MINOR (R6
continuation -- section-lead "Anticipated friction
points" still claimed entries were upcoming when
they were already locked) -> R8 APPROVE 0 findings.

Total: **12 distinct findings closed across R1-R7
(0B+12M+0N; 8 rounds)**. Lockability:yes sustained
across R1-R8 (no BLOCK findings). All findings were
spec-text consistency, cross-reference accuracy,
inter-cluster drift, recap-layer drift, and
framing-precision (§15.0's loose "DSL-rejected
ADTs" grouping that included non-Adt TyKinds) --
no architectural reframings. The R3 deeper-scope
fix corrected a stage-3/stage-9 framing imperfection
(§15.0's grouping of `Pat` / `UnsafeBinder` /
`Foreign` / `Coroutine family` under "DSL-rejected
ADTs" despite their being non-Adt TyKind variants)
under stage-14 enforcement-model hygiene scope; no
D-tag flip resulted from the fix.

Coverage by round:

| Round | Verdict | Findings (B+M+N) | Notes |
| ----- | ------- | ----------------- | ----- |
| R1    | MINOR   | 0+5+0 = 5         | Dangling Appendix A.6 reference at §13.2 vtable shim narrative; stale "§2 dual-backend rule" pointer (×2) in `generic-c-backend-snapshot.md`; "14th helper" inconsistency between §3.5.2 (13) and §3.6.8 (14) post-D-14-7.4/.5; stale §9.7 line-number refs in pretty-printer Bytes / fat-pointer per-arm tables; stale `_0 = const ()` operand prefix at six top-level render specimens (D-14-4.9 vs D-14-6 drift). |
| R2    | MINOR   | 0+1+0 = 1         | Stale "§2 'store the result, not the input' principle" ref at `generic-c-backend-snapshot.md` §7 terminator narrative -- third stale "§2" reference in that file. |
| R3    | MINOR   | 0+1+0 = 1         | **Deeper-scope fix**: §15.0 + §15.1 enforcement framing grouped non-Adt TyKinds (`Pat`, `UnsafeBinder`, `Foreign`, `Coroutine` family) as "special-ADTs". Narrowed §15.0 item 2 to actual ADTs only (`Box`, `c_void`, `Atomic*`); added "Non-ADT TyKind rejections" paragraph naming top-level TyKind match-arm dispatch as the enforcement gate for non-Adt variants. Mirrored fix in §15.1 enforcement-model paragraph. |
| R4    | MINOR   | 0+2+0 = 2         | Two more stale "line 4583" cross-references to the §15.1 UnsafeBinder row in §22 narrative + §22 rvalue rejection table; D-9-9 friction text at `stage-14/status.md` claimed "§23.6.1 9 -> 10 rows" but D-14-7.9 LOCKED text says it stays at 9 rows (table grows in §15.1, not §23.6.1). |
| R5    | MINOR   | 0+2+0 = 2         | D-14-7 recap surfaces still described Specimen 70 as carrying the D-14-9 forward-pointing parenthetical (D-14-9 had removed it); merger-time flip queue in `memory.md` Step 4 listed only 3 of the 5 queue entries (omitted D-9-10 + D-13-6 from earlier clusters). |
| R6    | MINOR   | 0+1+0 = 1         | `stage-14/status.md` Cross-stage friction section enumerated only 4 of the 5 queue entries (omitted D-12-1.3) + handoff bullet had stale "+ other anticipated D-tags" wording conflicting with the exact 5-entry queue. |
| R7    | MINOR   | (R6 continuation) | R6 partially still-open: Cross-stage friction section lead "Anticipated friction points still on watch for upcoming clusters" no longer matched the locked status of its three entries (all locked at D-14-7.9). |
| R8    | **APPROVE** | 0+0+0 = 0     | Close-out. All R6/R7 fixes verified closed; no new findings. |

**Trajectory pattern.** The cross-cluster review
ran 8 rounds (the longest single-pass review in
stage 14 -- exceeding D-14-6 / D-14-7's 6 rounds).
Driver: the cross-cluster review scope is
intrinsically broader than per-cluster review, and
hygiene drift accumulates across multiple touch
sites that no single cluster review can catch.
Particular drift categories observed:
1. **Stale line-number citations** (R1-#4, R4-#1):
   per-cluster spec edits push later lines down,
   leaving cross-references stale. Mitigation
   (now landed throughout the working files):
   replace specific line numbers with section
   anchors.
2. **Inter-cluster operand-prefix drift** (R1-#5):
   D-14-4.9's bare-operand rendering vs D-14-6's
   top-level render specimens carrying stale
   `const ` prefix from pre-D-14-4.9 conventions.
   Mitigation: replace_all surgical fix across the
   six specimen sites.
3. **Recap-layer drift** (R5-#1): forward-pointer
   parentheticals (e.g., D-14-7 -> D-14-9) become
   stale after the pointee cluster lands; recap
   surfaces that still describe the parenthetical
   as present create reader confusion. Mitigation:
   annotate "since removed by D-14-Y closing the
   cross-finding" in recap surfaces (preserving
   locked audit-shape historical content).
4. **Framing-precision drift** (R3): pre-existing
   stage-3/stage-9 framing looseness (§15.0's
   grouping of non-Adt TyKinds under "DSL-rejected
   ADTs") was inherited by stage-14's new
   enforcement-model paragraph. Cross-cluster
   review surfaced the inconsistency between the
   framing and the actual rustc TyKind grouping.
5. **Incomplete merger-time flip queue
   enumeration** (R5-#2, R6-#1): five-entry queue
   was not consistently enumerated across all four
   tracker surfaces (`status.md`, `stage-14/status.md`,
   `stage-14/memory.md`, `stage-14/discussions.md`).
   Mitigation: enumerate full 5-entry queue at each
   surface with explicit source-decision annotation.

**Cluster status**: per-stage cross-cluster review
LOCKED 2026-05-13 at R8 APPROVE. **Cross-stage
friction**: none new from the review pass (the R3
fix corrected an inherited framing imperfection;
no D-tag flips). **Phase 1 spec closure**: the
stage-14 spec surface is now ready for hand-off to
merger. Next: stage-14 merger close per README §5.5
7-step contract (integrate stage-14 discussions +
review-log + spec-snapshot + pretty-printer-snapshot
+ generic-c-backend-snapshot into top-level
artifacts; flip the five-decision REOPEN queue in
`decisions.md`; run post-merge Codex review per
§5.5 step 6; freeze stage-14 snapshots with
"Snapshot at close of stage 14" headers; update
top-level `status.md` to mark stage 14 closed).

Codex artifacts: prompts and outputs at
`/tmp/codex_cross_r1.txt` through
`/tmp/codex_cross_r8.txt`.
