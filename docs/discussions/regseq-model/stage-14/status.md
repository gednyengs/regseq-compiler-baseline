# Stage 14 status

Last updated: 2026-05-14. **STAGE 14 CLOSED (merger
contract executed 2026-05-13).** All eleven stage-14
cluster surfaces locked at Phase B APPROVE during
stage-14 work; per-stage cross-cluster review
closed at R8 APPROVE (8 rounds; 12 findings closed,
0B+12M+0N); merger close per README §5.5 7-step
contract landed 2026-05-13:

- **Step 1 + 2**: `stage-14/discussions.md` +
  `stage-14/review-log.md` appended to top-level
  artifacts under `## Stage 14` headers
  (mechanical concat).
- **Step 3**: All stage-14 D-tags (D-14-1 through
  D-14-10 + the no-D-tag `core::ffi::c_void`
  cluster) registered in `decisions.md` under a new
  `## Stage 14 (Phase-1 foundation extensions)`
  section.
- **Step 4**: Five-decision REOPEN queue flipped
  in `decisions.md`: D-9-9 (per D-14-7), D-9-10
  (per D-14-2.5), D-12-1.3 (per D-14-7), D-13-4.3
  (per D-14-7), D-13-6 (per D-14-4.9) -- each with
  the locked stage-14 revision annotation.
- **Step 5**: Three stage-14 snapshots lifted to
  top-level merger artifacts:
  - `stage-14/spec-snapshot.md` -> `spec.md`
    (13046 lines).
  - `stage-14/pretty-printer-snapshot.md` ->
    `pretty-printer.md` (4438 lines).
  - `stage-14/generic-c-backend-snapshot.md` ->
    `generic-c-backend.md` (new top-level file,
    755 lines, per D-14-10 carve-out + README §5.9
    rule 7).
- **Step 6**: Post-merge tier-3 Codex review on
  all four merger artifacts closed at R15 APPROVE
  / Lockability:yes / Close-out:YES after a
  15-round trajectory (1B+18M+0N; 19 findings
  closed). The R1 BLOCK was a wrong terminator
  vocabulary in `generic-c-backend.md` §7; R2-R14
  closed cross-reference accuracy, terminology
  hygiene, registry-vs-spec consistency, and
  co-equal-artifact framing drift. See top-level
  `review-log.md` `## Stage 14 / Post-merge
  tier-3 review` for the round-by-round trail.
- **Step 7**: Top-level `status.md` marks
  **STAGE 14 CLOSED** + announces phase-1
  implementation sweep #1 as the next major
  project step. This file + `spec-snapshot.md`,
  `pretty-printer-snapshot.md`, and
  `generic-c-backend-snapshot.md` are frozen
  with "Snapshot at close of stage 14" headers.

**Phase 1 spec is now closed.** All foundation
clusters delivered: Q7 intrinsic recognition +
DomainAttr sibling-table architecture (D-14-1 /
D-14-3); diagnostic-logging intrinsic vocabulary
(D-14-2); generic-C backend codegen carve-out
(D-14-10); composer-renderer trio
(`render_const` / `render_instance` / `render`
per D-14-4 / D-14-5 / D-14-6); external-review
Findings #2 / #3 / #4 (D-14-7 / D-14-8 / D-14-9);
`core::ffi::c_void` enforcement narrative.
Cross-stage friction queue exhausted at merger
close (all five entries flipped to `[REOPENED]`
in `decisions.md`).

**Cross-session pickup pointer for the next phase**:
the next major project step lives at top-level
`status.md` -- **Phase-1 implementation sweep #1**
(stages 1-14 in one pass; per README §2 "Spec
before implementation" amendment). This stage-14
folder is now historical; future spec work opens
**Stage 15** (regseq-trait scaffolding + Lifecycle
+ driver-struct discovery + RegseqError) per the
phase-2 queue in README §1.3 + §2 "Domain
extensions iterate per stage in phase 2".

Previous: 2026-05-13. **`core::ffi::c_void`
rejection enforcement cluster LOCKED at per-stage
Phase B R3 APPROVE / Lockability:yes / Close-out:YES
(2026-05-13).** 3-round Phase B trajectory closed
(2026-05-13): R1 MINOR 3 findings (0B+2M+1N;
Coroutine family double-classification in the new
§15.1 enforcement paragraph + stale line-number
citations at six audit + tracker sites after the
spec edit pushed lines down + dangling "Phase B."
fragment at the audit closure check) -> R2 MINOR
1 finding (0B+1M+0N; missed stale line number at
`memory.md` -- same class as R1-#2 but at a surface
not touched in the R1 fix sweep) -> R3 APPROVE 0
findings. **Total: 4 findings closed across R1-R2
(0B+3M+1N; 3 rounds)**. All findings were spec-text
consistency at the audit + tracker + spec-
introduction text surfaces, not architectural
reframings. Lockability:yes sustained across R1-R3
(no BLOCK findings). All Phase A content unchanged
through Phase B; the structural directive (two new
§15.1 introduction paragraphs naming the §15.0 gate
+ defining the opaque-struct pattern) is intact.
**Cluster status**: c_void enforcement Phase B
LOCKED 2026-05-13.
Pure spec hygiene cluster landing the enforcement
narrative at `spec-snapshot.md` §15.1 introduction.
Phase A drafting verified that the c_void row
already exists in the §15.1 rejection table
(`core::ffi::c_void` | type translation | recognised
via `tcx.is_lang_item(LangItem::CVoid)`; opaque-
struct pattern recommended), the §15.0 Special-ADT
dispatch-ordering rule already gates the rejection
algorithmically (the special-ADT check runs
unconditionally before generic struct/enum/union
translation per the §15.0 normative ordering
statement), and Appendix A.1 already walks the
`*mut c_void` -> `*mut OpaqueCtx` rewrite end-to-
end. **One applied edit**: a new "**Enforcement
model**" paragraph + an "**Opaque-struct pattern**"
paragraph added to the §15.1 introduction (after
the four-category narrative paragraph, before the
rejection table) explicitly naming which §15.1
rows the §15.0 gate enforces and defining the
opaque-struct pattern inline (`#[repr(C)]` ZST
struct with a single private zero-length-array
field; admitted via the ordinary `Ty::Struct`
path; lowers to incomplete-struct pointer at the
C-ABI boundary per §14.5 + §8.0 invariant 19). The
new paragraphs forward-reference Appendix A.1's
end-to-end rewrite. **No D-tag allocated** --
stage-3 D16 already covers the decision-level
commitment to reject c_void in v1; stage-3 D1.4
already recommends the opaque-struct pattern. **No
D-tag flip** -- stage-3 D16 and D1.4 are unchanged.
**No sub-decisions allocated** -- the enforcement-
model gap is structurally a single coherent edit at
one location. **Methodology rules invoked**: README
§5.9 rule 5 (no D-tag inline references in spec
text) + README §2 "C semantics with information
preservation" + README §2 "Forward expectations
live in `stage-N/expectations.md`". **No fresh
rustc evidence pass required** -- the recognition
mechanism + structural shape of the opaque-struct
pattern are already specified at §15.0 + §15.1 +
Appendix A.1. **Phase 1 spec closure**: this
cluster closes the **fourth and final stage-14
spec surface** (after D-14-7 / D-14-8 / D-14-9 +
the composer-renderer trio + the foundation
extensions). After Phase B closes, the per-stage
cross-cluster Codex review on the full
`spec-snapshot.md` + `pretty-printer-snapshot.md`
runs (anticipated 2-4 rounds), then hand-off to
merger per §5.5 7-step contract (flips D-9-10 +
D-13-6 + D-12-1.3 + D-9-9 + D-13-4.3 to
`[REOPENED]` in `decisions.md`). **Cross-stage
friction**: none new from this cluster. The five-
decision merger-time flip queue remains unchanged.
**Next concrete action**: run the per-stage cross-
cluster Codex review on the full
`spec-snapshot.md` + `pretty-printer-snapshot.md`
(anticipated 2-4 rounds; per README §5.5 step 6 +
§5.6 Tier-2 discipline this catches inter-cluster
drift across the entire stage-14 spec surface
before merger). After cross-cluster review closes
at APPROVE, hand off to merger per §5.5 7-step
contract.

Previous: 2026-05-13. **D-14-9 cluster LOCKED at
per-stage Phase B R2 APPROVE / Lockability:yes /
Close-out:YES (2026-05-13).** 2-round Phase B
trajectory closed (2026-05-13): R1 MINOR 2 findings
(0B+1M+1N; stale "three clusters" preamble wording
in `discussions.md` + audit overstatement of how
Specimen 70b differs from Specimen 73) -> R2
APPROVE 0 findings. **Total: 2 findings closed
across R1 (0B+1M+1N)**. Both findings were spec-
text consistency at the audit + preamble surfaces,
not architectural reframings. Lockability:yes
sustained across R1-R2 (no BLOCK findings). **No
cascade pattern** -- the two specimen edits had no
recap surfaces to drift; D-14-9's 2-round
trajectory matches D-14-8's shortest-trajectory
shape. **Cluster status**: D-14-9 Phase B LOCKED
2026-05-13. **Phase 1 spec closure**: D-14-9
closes the **third surgical-fix cluster** of stage
14 (external-review Finding #4; after D-14-7 /
Finding #2 and D-14-8 / Finding #3). Pure pretty-
printer-spec hygiene cluster landing the Finding
#4 split at `pretty-printer-snapshot.md` §9.9.
**Two applied edits**: (i) Specimen 70 input
downshifts `opaque_handle_eligible: true` to
`false` and the output drops the `#[opaque_handle]`
line, leaving a clean Function attribute-stress
specimen; the D-14-7 cross-finding parenthetical
forward-pointing to D-14-9 is also removed.
(ii) new Specimen 70b carrying
`kind: ItemKind::Struct, def_path: <pl011::Handle>,
opaque_handle_eligible: true` renders
`#[opaque_handle]` above `struct pl011::Handle`,
preserving coverage of the bridge-outcome rendering
on the only `ItemKind` that can carry the flag per
§8.0 invariant 19. Specimen count drifts ~75 -> ~76
via the 70b suffix; gap-free 70 -> 70b -> 71
sequence acceptable per D-13-7.2's policy-bucket-
exception spirit. **No sub-decisions allocated**;
**No D-tag flip** -- D-13-7's decision text is not
revised. **Cross-stage friction**: none new from
D-14-9. The five-decision merger-time flip queue
remains unchanged at D-9-10 (D-14-2.5) + D-13-6
(D-14-4.9) + D-12-1.3 + D-9-9 + D-13-4.3
(D-14-7.9). Remaining stage-14 work:
**`core::ffi::c_void` rejection enforcement** (no
new D-tag; stage-3 D16 already specifies) + per-
stage cross-cluster Codex review on the full
`spec-snapshot.md` + `pretty-printer-snapshot.md`
(anticipated 2-4 rounds), then hand-off to merger
per §5.5 7-step contract.

Previous: 2026-05-13. **D-14-8 cluster LOCKED at
per-stage Phase B R2 APPROVE / Lockability:yes /
Close-out:YES (2026-05-13).** 2-round Phase B
trajectory closed (2026-05-13): R1 MINOR 2 findings
(0B+2M+0N; tracker line-number wording at three
surfaces + placeholder `D-9-?` in audit cross-stage
friction section) -> R2 APPROVE 0 findings. **Total:
2 findings closed across R1 (0B+2M+0N)**. Both
findings were spec-text consistency at the audit +
tracker surfaces, not architectural reframings.
Lockability:yes sustained across R1-R2 (no BLOCK
findings). **No cascade pattern** -- the
single-paragraph edit had no recap surfaces to
drift; the 2-round trajectory is the shortest in
stage 14 so far (matches what Phase A trackers
anticipated). **Cluster status**: D-14-8 Phase B
LOCKED 2026-05-13. **Phase 1 spec closure**: D-14-8
closes the **second surgical-fix cluster** of stage
14 (external-review Finding #3). Pure spec hygiene
cluster landing the Finding #3 retarget at
`spec-snapshot.md` §9.2 paragraph 3 (the closing
clause of the `Struct / Enum / Union`
`emission_name` bridge-construction-rule bullet,
now committing phase-1 to determinism + uniqueness
only and forwarding the full normative C-identifier
construction algorithm to the C-backend spec's
responsibility as out of phase-1 scope). **No
sub-decisions allocated** -- the directive is fully
prescribed by `expectations.md` Finding #3 (verbatim
replacement text). **No D-tag flip** -- stage-9
`emission_name` field shape, identity rule, and
uniqueness guarantee all unchanged. **Cross-stage
friction**: none new from D-14-8. The five-decision
merger-time flip queue remains unchanged at D-9-10
(D-14-2.5) + D-13-6 (D-14-4.9) + D-12-1.3 + D-9-9 +
D-13-4.3 (D-14-7.9).

Previous: 2026-05-13. **D-14-7 cluster LOCKED at
per-stage Phase B R6 APPROVE / Lockability:yes /
Close-out:YES (2026-05-13).** 6-round Phase B
trajectory closed (2026-05-13): R1 MINOR 3 findings
(0B+3M+0N) -> R2-R5 each MINOR 1 finding per round
(recap-text-duplication cascade mirroring D-14-6's
6-round trajectory exactly) -> R6 APPROVE 0 findings.
**Total: 7 findings closed across R1-R5
(0B+7M+0N)**. All 9 D-14-7 sub-decisions remain
LOCKED at Phase A unchanged -- Phase B revisions
were spec-text consistency (3 R1 findings on
field-count / audit-header label / Q5 ALLOW_LIST +
4 rounds of recap-text-duplication cascade), not
architectural reframings. Lockability:yes sustained
across R1-R6 (no BLOCK findings). The R5 cascade-
breaking fix (bumping the four lagging surfaces to
the full R5 trajectory at once including R5 itself)
ended the recap-text-duplication cascade in one
pass; R6 confirmed APPROVE.
**Cluster status**: D-14-7 Phase B LOCKED
2026-05-13. **Phase 1 spec closure**: D-14-7 closes
the first surgical-fix cluster of stage 14
(external-review Finding #2). The composer-renderer
trio (D-14-4 / D-14-5 / D-14-6) plus the first
surgical fix (D-14-7) are now fully specified.
**Cross-stage friction**: three new REOPENs from
D-14-7 (D-12-1.3, D-9-9, D-13-4.3) join the pending
merger-time flips from prior clusters (D-9-10 from
D-14-2.5, D-13-6 from D-14-4.9). The five-decision
merger-time flip queue will land at stage-14 merger
close per §5.5 step 4.

Previous: 2026-05-13. **D-14-7 cluster LOCKED at
per-stage Phase A (9/9 sub-decisions).** Phase A
drafting session (2026-05-13) locked all 9 D-14-7
sub-decisions covering the external-review Finding #2
surface (`#[naked]` rejection in v1): **D-14-7.1**
(load-bearing -- reject at Item translation;
diagnostic names `#[naked]` + recommends future
phase-2 stage-17 `#[rseq::interrupt(...)]` deferral
path); **D-14-7.2** (recognition API:
`tcx.codegen_fn_attrs(did).flags.contains(CodegenFnAttrFlags::NAKED)`
during stage-12 D-12-5 Phase 2 Translation+Synthesis);
**D-14-7.3** (rejection-table placement: new "Item
translation" phase row at `spec-snapshot.md` §15.1
extending the header note to acknowledge a fourth
rejection category; sibling to stage-9 D-9-9 row 8 at
§23.6.1); **D-14-7.4 + D-14-7.5** (`Item.is_naked`
field + `instance_is_naked` helper deletions at §8.0
+ §3.5.2; 14-helper inventory downshifts to 13);
**D-14-7.6** (pretty-printer per-flag table edits at
§5.3 + §5.6 row-order recap + §5.1 atomic-shape
example list + §5.2 default-suppression narrative);
**D-14-7.7** (specimen alignment at §9.9: Specimens
69 + 70 + 75 input shapes drop `is_naked`; Specimen
70 output drops `#[naked]` rendered line;
cross-finding parenthetical referencing D-14-9 /
Finding #4 added as hygiene -- since removed by
D-14-9 closing the cross-finding); **D-14-7.8** (§3.6.6 paragraph 3
finalisation from D-14-7-territory pre-lock wording
to Stage-14-D-14-7 locked-state wording);
**D-14-7.9** (cross-stage friction outcome -- three
new REOPENs at merger close: D-12-1.3 + D-9-9 +
D-13-4.3 with the documented annotations per
`expectations.md` Finding #2). Step 4 rustc evidence
pass landed at `prior-art/rustc-naked.md` (6 Q-tags
Q1-Q6) + `rustc-references.md` "`#[naked]` handling"
section (5 file:line landmarks:
`rustc_attr_parsing/src/attributes/codegen_attrs.rs:221-333`,
`rustc_middle/src/middle/codegen_fn_attrs.rs:167`,
`codegen_fn_attrs.rs:14-28`,
`rustc_codegen_ssa/src/mir/naked_asm.rs:30-41`,
`rustc_codegen_ssa/src/mono_item.rs:44-51` + `:74-82`).
Spec contributions landed:
`spec-snapshot.md` §3.5.2 + §3.6.6 + §8.0 + §15.1
edits; `pretty-printer-snapshot.md` §5.1 + §5.2 +
§5.3 + §5.6 + §9.9 edits. **Cross-stage friction**:
three new REOPENs from D-14-7 (D-12-1.3, D-9-9,
D-13-4.3) join the pending merger-time flips from
prior clusters (D-9-10 from D-14-2.5, D-13-6 from
D-14-4.9). **Codex Phase B trajectory (in progress)**:
R1 MINOR / Lockability:yes / Close-out:NO 3 findings
(0B+3M+0N; §8.0 field-count + D-14-7 audit pre-lock
header + Q5 ALLOW_LIST; all closed) -> R2 MINOR /
Lockability:yes / Close-out:NO 1 finding (0B+1M+0N;
recap-text-duplication tail -- §3 + memory.md; all
closed) -> R3 MINOR / Lockability:yes / Close-out:NO
1 finding (0B+1M+0N; four lagging surfaces at R1
trajectory; all closed) -> R4 MINOR / Lockability:
yes / Close-out:NO 1 finding (0B+1M+0N; same four
surfaces shifted one round -- R3 bumped to R2-
trajectory wording but lagged canonical R3; all
closed) -> R5 MINOR / Lockability:yes / Close-out:NO
1 finding (0B+1M+0N; same shift forward -- R4
bumped to R3-trajectory wording but lagged canonical
R4; **cascade-breaking fix applied -- the four
lagging surfaces bumped to the full R5 trajectory
at once including R5 itself + "Phase B R6 close-out
pending" rather than stopping at R4**; all closed).
Cluster crossed Lockability:yes at R1 (no BLOCK
findings; architectural picture stable since R1).
**Next concrete action**: kick off Phase B R6
close-out Codex review verifying R5 fixes land
cleanly + final close-out check. R6 anticipated at
0 findings APPROVE / Close-out:YES (the cascade-
breaking fix synchronises the four lagging surfaces
with the canonical R5 trajectory; D-14-6's R6
closed at zero findings under the same pattern).

Previous: 2026-05-13. **D-14-6 cluster LOCKED at
Phase B R6 APPROVE / Lockability:yes / Close-out:YES.**
6-round Phase B trajectory closed (2026-05-13):
R1 BLOCK 6 findings (2B+4M+0N) -> R2 MINOR /
Lockability:yes 3 findings (0B+3M+0N) -> R3 MINOR /
Lockability:yes 1 finding (0B+1M+0N) -> R4 MINOR /
Lockability:yes 2 findings (0B+2M+0N) -> R5 MINOR /
Lockability:yes 2 findings (0B+2M+0N) -> R6 APPROVE
0 findings. Total: **14 distinct findings closed
across R1-R5 (2B+12M+0N)**. All 6 D-14-6 sub-decisions
remain LOCKED at Phase A unchanged -- Phase B revisions
were spec-text consistency (recap-text-duplication
cascade), not architectural reframings. **No stage-13
D-tag REOPENs introduced** by any R1-R5 fix (D-14-6.6
zero-REOPEN claim holds across the entire trajectory).
**Cluster status**: D-14-6 Phase B LOCKED 2026-05-13.
**Phase 1 spec closure**: D-14-6 closes the third and
final composer-renderer cluster per stage-13 D-13-4.6;
the composer trio (`render_const` / `render_instance`
/ `render`) is now fully specified.

Previous: 2026-05-13. **D-14-6 cluster Phase A
LOCKED + Phase B R5 MINOR / Lockability:yes /
Close-out:NO.** Phase A drafting
session (2026-05-13) locked all 6 D-14-6 sub-decisions
(**D-14-6.1** API signature `render(&self) -> String`
zero-argument; **D-14-6.2 (load-bearing)** iteration
order -- walk `model.instances.iter_enumerated()` in
canonical F4-2 order per `spec.md` §16.3, no
render-time re-sort; **D-14-6.3** per-Instance
separator -- top-level composer emits `"\n\n"`
between consecutive `render_instance(id)` outputs
(which end at `}` or `;` with no trailing newline),
no leading or trailing separator, first-flag pattern
adapted from rustc's `write_mir_pretty` at
`pretty.rs:328-335`; **D-14-6.4** file-level
envelope -- no header / no trailer / no metadata
wrapper, output starts at first rendered byte of
first `render_instance` output (Stream-1 + Stream-2
attribute block per §10.2.7 when non-empty, otherwise
kind-keyword line) and ends at last rendered byte
(`}` or `;`) with no trailing newline injected;
**D-14-6.5** empty-model case -- empty `String` `""`
identity (loop never runs, no separator emitted);
**D-14-6.6** cross-stage friction outcome -- zero
new stage-13 D-tag REOPENs from D-14-6 drafting,
pending merger-time flips from prior clusters
unchanged). 5 specimens (129-133) added at
`pretty-printer-snapshot.md` §9.12 covering: empty
model, single-Instance Function, multi-Instance
mixed rank-group, multi-Instance same-rank-group
(Functions including foreign + intrinsic-attributed),
full per-rank coverage Function/Static/Const/Struct/
Enum/Union. Spec contributions: new
`pretty-printer-snapshot.md` §10.3 (7 subsections
§10.3.1-§10.3.7); §1.3 composer listing finalised;
§8 ownership-table row finalised; §9.12 specimens;
new `stage-14/prior-art/rustc-render.md` (6 Q-tags)
+ `rustc-references.md` Top-level render section
(4 file:line landmarks). **Codex Phase B trajectory
(in progress)**: R1 BLOCK 6 findings (2B+4M+0N) ->
R2 MINOR / Lockability:yes / Close-out:NO 3 findings
(0B+3M+0N; recap-text-duplication cascade) -> R3 MINOR
/ Lockability:yes / Close-out:NO 1 finding (0B+1M+0N;
residual pre-lock planning text in `discussions.md`
D-14-6 cluster opening Question paragraph +
`memory.md` D-14-6 agenda; both surfaces non-canonical
recap, not load-bearing) -> R4 MINOR / Lockability:yes
/ Close-out:NO 2 findings (0B+2M+0N; §10.3.7 worked-
example output trailer claimed implicit trailing `\n`
contradicting §10.3.4 + missing function-body blank
line; `stage-14/status.md` Next action trajectory
sentence stale at R2 with "R3 close-out pending"
language) -> R5 MINOR / Lockability:yes /
Close-out:NO 2 findings (0B+2M+0N; D-14-6.3 and
D-14-6.4 audit worked examples in `discussions.md`
still missing function-body envelope blank line that
R4 added to canonical §10.3.7; three recap surfaces
-- `stage-14/status.md` Next action cluster-state
recap + `discussions.md` D-14-6 heading + `memory.md`
D-14-6 overview -- still stopped at R3/R4 instead of
R5). R1's
2 BLOCKs were the load-bearing fixes: (1) separator
algorithm re-architected to make the top-level
composer own the `"\n\n"` bytes (R1 had assumed
each `render_instance` carried an implicit trailing
`\n`, but §9.11 specimens establish the
no-trailing-newline contract; resolved without
reopening D-14-5); (2) §10.3.4 envelope wording
corrected to acknowledge the Stream-1 + Stream-2
attribute block as the first rendered line (not just
the keyword line; Specimen 132 was the falsifying
case). R1's 4 MINORs were D-tag wording strip + hex
Uint in discussions worked examples + codegen-
ordering overstatement (default sorts by symbol-name
only; span-key gated on `-Z codegen-source-order`) +
const-items mis-classified as empty-`mir_keys`
example. R2's 3 MINORs are all recap-text-duplication
of R1's load-bearing fixes (D-14-6.1 sketch +
D-14-6.3 audit narrative + prior-art Q3/Q4 reasoning
+ D-14-6.4 audit + cluster opening D-14-6.2
sub-decision allocation). R3's 1 MINOR was the
residual cascade R1+R2 missed in two non-canonical
recap surfaces (`discussions.md` D-14-6 cluster
opening Question paragraph still said §10.3 was
"currently a stub"; `memory.md` D-14-6 agenda still
framed D-14-6 as "next" with "Pre-staged sub-decision
inventory"); both rewritten to post-lock state.
**Cross-stage friction**: none new from D-14-6
R1+R2+R3+R4+R5 (D-14-6.6 zero-REOPEN claim holds).
Pending merger-time flips unchanged: D-9-10 (from
D-14-2.5), D-13-6 (from D-14-4.9), plus D-12-1.3 /
D-9-9 / D-13-4.3 (from D-14-7 when locked). **Next
concrete action**: kick off Phase B R6 close-out
Codex review verifying R5 fixes land cleanly + final
close-out check; on R6 APPROVE / Close-out:YES the
cluster closes Phase B.

Previous: 2026-05-13. **D-14-5 cluster LOCKED
at Phase B R4 APPROVE / Lockability:yes /
Close-out:YES.** 4-round Phase B trajectory closed
(2026-05-13): R1 BLOCK (3B+3M+1N; row-order recap
divergence + hex-vs-decimal Static specimens +
arg-local-redeclaration outline bug + declaration-
trailer wording + escaped-quotes input + stale
forward-pointers + unqualified "byte-identical"
wording) -> R2 BLOCK (1B+2M+1N; §10.2 intro
third-source-of-truth row-order continuation + 7
stale "pending/deferred" wordings + per-variant
attribute-block cross-reference gap at §10.2.4/5/6
+ status.md prior-entry continuation) -> R3 MINOR
Lockability:yes (0B+2M+0N; 5 more
"defers/will pin/may/in progress" wordings + §3.3
sigil row contradicting D-14-5.7
composer-from-scratch) -> R4 APPROVE 0 findings.
Total: **13 distinct findings closed across R1-R3
(4B+7M+2N; 5 R1/R2/R3 continuations + 8 fresh)**.
All 8 D-14-5 sub-decisions remain LOCKED at Phase
A unchanged -- Phase B revisions were spec-text
consistency (recap-text-duplication pattern
matching D-14-4), not architectural reframings.
**No stage-13 D-tag REOPENs introduced** by any
R1-R3 fix; the §5.6 helper factor-out (D-14-5.7) is
additive spec text within §5, not a §5.3 / §5.5
table modification. **Cluster status**: D-14-5
Phase B LOCKED 2026-05-13. **Cross-stage friction**:
none new from D-14-5. Pending merger-time flips
unchanged from prior clusters: D-9-10 (from
D-14-2.5), D-13-6 (from D-14-4.9), plus D-12-1.3 /
D-9-9 / D-13-4.3 (from D-14-7 when locked).
**Next concrete action**: open the next phase-1
cluster -- **D-14-6** (top-level `render(&self) ->
String` composer renderer) -- the third and final
composer-renderer cluster per stage-13 D-13-4.6.
D-14-6 iterates `model.instances` in Q2
codegen-keyed order and composes per-Instance via
the now-locked `render_instance` from D-14-5.

Previous: 2026-05-13. **D-14-5 cluster Phase B
R3 MINOR / Lockability:yes** -- Codex review closed
2026-05-13 with verdict `MINOR / Lockability: yes /
Close-out: NO`. 2 findings (0 BLOCK + 2 MINOR + 0
NIT): 1 continuation of R2.2 (five more stale
"defers/will pin/may/in progress" wordings in the
audit narrative at D-14-5.2 Aspect 1, D-14-5.2
spec-impact, D-14-5.6 Reasoning point 2, D-14-5.8
Next steps, §3 status-summary) + 1 new finding
(§3.3 sigil convention table `InstanceId` row at
`pretty-printer-snapshot.md:245` said
`render_instance` "composes `render_item` wrapper +
per-Instance content", contradicting D-14-5.7
LOCKED composer-from-scratch; the row was a
stage-13 D-13-4.6 forward-pointer that R1/R2 had
finalised at §1.3 + §8 + §10.2 but missed at §3.3).
All 2 findings closed at this pass: audit narrative
spots updated to LOCKED-state wording; §3.3 row
updated to "per D-14-5.7 LOCKED, composes
`render_def_path` + per-`InstanceKind` content +
the §5.6 shared `render_item_attribute_block`
helper for Stream 1; does NOT call `render_item`".
**Cluster crossed Lockability:yes at R3**: BLOCK
findings exhausted; no specimen correctness issues;
no stage-13 REOPENs; architectural picture holds.
Close-out withheld pending R4 verification of R3
fixes + final close-out check. R1+R2+R3 trajectory
matches D-14-4's recap-text-duplication phenomenon
(each round catches additional spots the prior
round missed). **Next concrete action**: kick off
Phase B R4 close-out Codex review verifying R3
fixes land cleanly + final close-out check; on
APPROVE / Close-out:YES, the cluster closes Phase
B and Phase A LOCKED status finalises.

Previous: 2026-05-13. **D-14-5 cluster Phase B
R2 BLOCK** -- Codex review closed 2026-05-13 with
verdict `BLOCK / Lockability: no / Close-out: NO`.
4 findings (1 BLOCK + 2 MINOR + 1 NIT): 3
continuations of R1 (R1.1 row-order recap drift in
the §10.2 intro third location, R1.6 stale
"pending/deferred" wording at 7 audit-narrative
spots that R1 missed, R1.7 unqualified
"byte-identical" wording in a prior status entry)
+ 1 new finding (§10.2.4 Struct / §10.2.5 Enum /
§10.2.6 Union per-variant subsections used
`[<attribute-block>\n]` in composition forms but
did not carry the explicit "Attribute block"
cross-reference to §10.2.7 + §5.6 that §10.2.1 /
§10.2.2 / §10.2.3 carry). All 4 findings closed at
this pass: §10.2 intro parenthetical rewritten to
point at §5.3 + §5.6 instead of re-listing field
order (removes the third source of truth);
cluster-opening + D-14-5.2 worked-examples
narrative + D-14-5.2 illustrative-example
code-comments + D-14-5.3 / D-14-5.4 / D-14-5.5
attribute-block aspects + D-14-5.8 Next steps all
flipped to LOCKED-state wording; new "Attribute
block" paragraphs added to §10.2.4 / §10.2.5 /
§10.2.6 mirroring the §10.2.1 / §10.2.2 / §10.2.3
pattern; `status.md:186` wording qualified to
"byte-identical Stream 1 (Item-flag) attribute
streams". No stage-13 D-tag REOPENs introduced.
R1+R2 pattern matches D-14-4's recap-text-
duplication phenomenon (R1 catches canonical
locations; R2 catches audit-narrative +
illustrative-example + status spots). **Next
concrete action**: kick off Phase B R3 Codex
review verifying R2 fixes land cleanly + checking
for further regression-continuation patterns.

Previous: 2026-05-13. **D-14-5 cluster Phase B
R1 BLOCK** -- Codex review closed 2026-05-13 with
verdict `BLOCK / Lockability: no / Close-out: NO`.
7 findings (3 BLOCK + 3 MINOR + 1 NIT): (1) §5.6
helper row-order recap + D-14-5.6 audit-shape
listing diverged from locked §5.3 table-row order
(swapped lang-flags and link_section); fixed in
both surfaces to match §5.3 order linkage ->
inline_hint -> is_naked -> lang-flags ->
link_section -> opaque_handle_eligible. (2)
Specimen 118 + §10.2.2 Static worked example +
D-14-5.3 audit worked example rendered initializer
value `0x10009000_u32` (hex form), violating D-14-4.11's
decimal-only `render_const` Uint rendering rule;
replaced with decimal form `268472320_u32` (5
occurrences across 2 files). (3) D-14-5.2
composition outlines (definition form in
`pretty-printer-snapshot.md` §10.2.1 + discussions
audit) included a `let mut _1: T1; // arg-local 1`
declaration line that contradicts §3.1 / D-13-2
(arg-locals appear ONLY in the header arg-list,
not in the locals declaration block); fixed to
show `_0: <ret>` then `_M: T` (first non-arg
local, where `M == arg_count + 1`) with an
explanatory inline comment. (4) Declaration-form
trailer wording ("immediately after the closing
`)` of arg-list") inconsistent with always-render
`-> Ret` rule; corrected to "immediately after
the return-type segment `-> <ret>`". (5) Specimen
112 `link_section` input value had escaped quotes
inside the stored string (`"\".text.boot\""`);
corrected to `<StringId resolving to ".text.boot">`.
(6) Stale §11 cross-reference said "render_instance
once shipped (§10.2)" + §3 status-summary entry
still listed "~10-15 specimens" as remaining work;
both retargeted to the locked-shipped state. (7)
NIT: "byte-identical attribute streams" wording in
§5.6 + §8 + cluster recaps was technically broader
than the actual Stream-1-only contract; qualified
to "byte-identical Stream 1 (Item-flag) attribute
streams" at 5 occurrences across
`pretty-printer-snapshot.md` (§5.6, §8 `render_item`
row, §10.2.x prose) + `discussions.md` (D-14-5
cluster opening + D-14-5.7 audit), with an explicit
"`render_item` does NOT emit Stream 2 domain-attribute
rows; those live at the Instance-level composer per
§10.2.7" note where the broader wording previously
stood. **All 7 R1 findings closed at this pass
(3 BLOCK + 3 MINOR + 1 NIT) across
pretty-printer-snapshot.md §5.6 + §8 + §10.2.1 +
§10.2.2 + §10.2.7 + §11 + Specimen 112/118
inputs/outputs; discussions.md D-14-5.2 + D-14-5.6 +
D-14-5.7 audit-shape entries + §3 status-summary.**
**Next concrete action**: kick off Phase B R2 Codex
review on the corrected D-14-5 spec contributions
(verify R1 fixes land; check for cascading findings).

Previous: 2026-05-13. **D-14-5 cluster
specimens added to §9.11** -- 19 specimens
(Specimen 110-128) covering each `InstanceKind`
variant + edge cases per D-13-7 specimen
convention. Specimens exercise: Function plain
(110), Function unsafe with raw pointer (111),
Function with full Stream-1 attribute block (112),
Function foreign C declaration / body=None (113),
Function foreign C variadic (114), Function with
diagnostic-logging intrinsic / Stream-1 +
Stream-2 (115), bridge-synthesised drop glue (116),
divergent return Ty::Never (117); Static defined
immutable (118), Static defined mutable with
struct initializer (119), Static foreign extern
init=None (120); Const scalar (121), Const
reference-typed &str (122); Struct unit-form
empty (123), Struct tuple-form synthetic-`_N`
field names (124), Struct named-field with
lang-flag + generic args (125); Enum
EnumShape::Multiple mixed ctor (126), Enum
EnumShape::Empty (127); Union simple
named-field (128). Each specimen records Input
(structured Instance + Item state) and Output
(rendered text); annotated with the D-14-5.*
sub-decision arm exercised. Specimen numbering
continues from D-14-4's §9.10 (Specimens 76-109).

**Cluster status**: D-14-5 cluster Phase A
architecturally LOCKED + specimens added (8/8
sub-decisions + 19 specimens). **Next concrete
action**: Phase B Codex review on the full
`pretty-printer-snapshot.md` D-14-5 contributions
(§10.2.1-§10.2.7 + §5.6 helper subsection + §9.11
specimens + §8 / §1.3 finalisation +
`spec-snapshot.md` §9.4 cross-reference paragraph).
Anticipated 2-4 rounds per the D-14-4 cluster's
4-round trajectory for similarly-sized
composer-renderer surface.

Previous: 2026-05-13. **D-14-5 cluster Phase A
LOCKED -- 8/8 sub-decisions LOCKED**. D-14-5.8
(cross-stage friction outcome) closes the
architectural pass with **zero new stage-13 D-tag
REOPENs from D-14-5 drafting**: the per-sub-decision
audit pass (D-14-5.1 through D-14-5.7) consumed all
stage-13 / earlier-stage spec surfaces (§3.1 body
envelope, §5.3 per-flag table, §5.4 per-`ItemKind`
wrapper, §5.5 block layout, §6.4 ADT use-site
form, §6.5 fn-pointer rendering, §11 Layout
schema, §3.4 sigil convention, §3.6 DomainAttr
tables) without modifying their locked content.
The §5.6 shared `render_item_attribute_block`
helper subsection (added by D-14-5.7) is **additive
spec text**, analogous to D-14-4 adding §10
Composer renderers without REOPENing stage-13's
§6 / §7 sections. Pending merger-time flips from
prior clusters unchanged: D-9-10 (from D-14-2.5),
D-13-6 (from D-14-4.9); plus D-12-1.3 / D-9-9 /
D-13-4.3 (from D-14-7 when locked). If Phase B
Codex review surfaces stage-13 REOPEN-shaped
findings, D-14-5.8 entry updates per "Spec is
mutable" -- v1 records the friction outcome as
of architectural closure.

**Cluster status**: D-14-5 cluster Phase A LOCKED
(8/8 sub-decisions). Remaining cluster work:
specimens (~10-15 in `pretty-printer-snapshot.md`
§9.11 per D-13-7 convention) + Phase B Codex
review (anticipated 2-4 rounds per the D-14-4
trajectory). **Next concrete action**: add
specimens to §9.11 covering each `InstanceKind`
variant + edge cases (Function defn / Function
foreign decl / Function with intrinsic attr /
Function with C-variadic / Static defined /
Static foreign / Const scalar / Const struct /
Struct unit / Struct tuple / Struct named-field /
Struct synthetic closure-captures / Enum empty /
Enum single / Enum multi-variant mixed ctor /
Union simple).

Previous: 2026-05-13. **D-14-5.6 + D-14-5.7
LOCKED at per-stage Phase A** -- attribute-block
composition rule (D-14-5.6) and `render_item` vs
`render_instance` layering boundary (D-14-5.7), the
final architectural locks of the D-14-5 cluster
(barring D-14-5.8 friction outcome).

**D-14-5.6 (attribute block composition rule).**
**Two-stream interleaving** in fixed order:
Stream 1 (Item-flag stream, §5.3 table-row order
via the §5.6 shared helper) precedes Stream 2
(`domain_attrs.item_attrs[item]` Vec-index order
per `spec.md` §3.6.2 D-14-3.3 bridge-walk-order
determinism). Each `#[...]` on its own line;
trailing `\n` when non-empty. Per-`AttrData`
variant render rule for stage-14 vocabulary:
`Intrinsic(IntrinsicKind::Log(LogLevel::{Debug,
Info, Warn, Error}))` -> `#[rseq::intrinsic("log-{debug,
info, warn, error}")]` with dash-form NAME from
`spec.md` §3.6.3 `IntrinsicKind::from_name`
reversed. Phase-2 stages (Mmio / Interrupt /
Atomic per stages 16-18) append per-family render
rules per the §3.6.2 closed-append-only-union
contract; stage-14 commits only the `Intrinsic`
variant rendering. Item-flag-stream-first ordering
chosen over domain-first / lexical / source-
preservation alternatives because rustc-derived
CodegenFnAttrs / AdtFlags attributes are the
first-class toolchain decorations a Rust source
author writes; tool-namespace attributes
(`#[rseq::intrinsic(...)]`, `#[clippy::...]`,
`#[serde(...)]`) typically follow as domain-
specific extensions -- the interleaved order reads
as the typical author-intent pattern.

**D-14-5.7 (layering boundary).** **Option (a)
composer-from-scratch LOCKED.** `render_instance`
does **not** call `render_item`; instead composes
the full per-variant envelope directly,
sharing only the §5.6 internal helper
`render_item_attribute_block(item: ItemId) -> String`
factored from §5.3 / §5.5. The helper produces the
Stream 1 portion of the attribute block; both
`render_item` (standalone-Item view per §5.4) and
`render_instance` (composer per §10.2 per-variant
subsections via §10.2.7) consume the same helper
for byte-identical Stream 1 (Item-flag) attribute
streams; `render_item` does NOT emit Stream 2
domain-attribute rows. Stage-13's public
`render_item(ItemId)` signature unchanged.
Driver: D-14-5.2's `extern "ABI"` decoration
forces Function-envelope composition outside
`render_item`'s ItemId-only context (rejecting
option (b) §5.4 signature extension); D-14-5.5's
ADT body composition + D-14-5.3 / D-14-5.4 Static
/ Const trailers add symmetric per-variant
composers; sharing the helper preserves
consistency without API duplication.

**Spec contributions**:
- `pretty-printer-snapshot.md` §10.2.7 new
  subsection "Attribute block composition" with
  two-stream interleaving rule + Stream 1 (§5.6
  helper) + Stream 2 (per-`AttrData` variant
  render table for stage-14 vocabulary) +
  multi-line layout convention + empty-case
  treatment + 5 worked examples covering
  no-attrs / Stream-1-only / both-streams /
  foreign-decl / lang-flag-Struct.
- `pretty-printer-snapshot.md` §5.6 new subsection
  "Shared attribute-block helper
  (`render_item_attribute_block`)" pinning the
  factored helper as the composition source
  consumed by both `render_item` (§5.4) and
  `render_instance` (§10.2 per-variant subsections
  via §10.2.7 Stream 1). §5.3 / §5.5 row order +
  block layout unchanged.
- `pretty-printer-snapshot.md` §10.2 intro outline
  updated -- §10.2.7 added to the locked-
  subsections list + layering-boundary paragraph
  announcing D-14-5.7's composer-from-scratch
  + shared-helper architecture.
- `pretty-printer-snapshot.md` §10.2.1 / §10.2.2 /
  §10.2.3 attribute-block aspect text updated
  from "composition rule deferred to D-14-5.6"
  to "per §10.2.7 attribute-block composition
  rule (D-14-5.6 LOCKED)" cross-references.
- `pretty-printer-snapshot.md` §8 per-method
  ownership table -- `render_instance(InstanceId)`
  row Description column finalised: "composes
  `render_def_path` + `render_ty` + body-level
  renderers + `render_const` + the §5.6 shared
  `render_item_attribute_block` helper per §10.2
  (per D-14-5.7 LOCKED, does NOT call
  `render_item`)". `render_item(ItemId)` row
  Description gains the shared-helper note.
  `render_item_attribute_block(ItemId)` new row
  added as stage-14 internal helper.
- `pretty-printer-snapshot.md` §1.3 composer
  listing for `render_instance` rewritten to
  reflect the composer-from-scratch surface +
  the per-variant composition map +
  cross-references to §10.2.1-§10.2.7.

**No D-13-2 / D-13-4 / D-13-4.3 / D-13-4.5 / D-13-6
REOPENs.** The §5.6 helper factor-out is internal
spec-text reorganization within §5; stage-13's
public APIs + per-flag table content are unchanged.

**Cluster status**: D-14-5 cluster Phase A drafting
in progress (7/8 sub-decisions LOCKED). All
architectural locks done. **Next concrete action**:
lock D-14-5.8 (cross-stage friction outcome -- the
final pass after all spec text lands; confirms no
REOPENs surfaced during D-14-5.1 through D-14-5.7
drafting per the cluster opening's working assumption).
Then specimens (~10-15 in §9.11) and Phase B Codex
review.

Previous: 2026-05-13. **D-14-5.5 LOCKED at
per-stage Phase A** -- ADT variants (Struct / Enum /
Union) rendering, the **load-bearing architectural
choice with no rustc precedent**. **Option B (full
definition body from `spec.md` §11 Layout) locked**;
Option A (header-only) rejected per the prior-art
Q7 evidence preponderance (matches §2.1 master
Rust-source-style; surfaces §11 Layout structure
consistently; supports field-level snapshot
diagnostics). Three per-variant arms:
- **Struct**: dispatch on `(fields.len(), first-field-name-pattern
  per §3.4)` -- empty -> `struct PATH<args>;` (unit
  form, collapses Rust's three syntactic empty-struct
  shapes); all-`_<idx>` synthetic field names -> tuple
  form `struct PATH<args>(T0, T1, ...);`; named source
  fields -> named-field form `struct PATH<args> {
  name: T, ... }`.
- **Enum**: dispatch on `EnumShape` per §11.1 --
  `Empty` -> `enum PATH<args> {}` zero-variant
  body; `Single { variant }` -> single arm inside
  `{ ... }`; `Multiple { variants }` -> per-variant
  arms inside `{ ... }`, comma-separated, trailing
  comma. Per-variant ctor form per same three-case
  dispatch as Struct (unit / tuple / named-field
  per §3.4).
- **Union**: named-field form unconditionally (Rust
  unions only carry named fields).

Common decoration: attribute block per D-14-5.2
contract (stage-14 has no `AttrData::Intrinsic`
targeting ADT Items in v1; reduces to §5.3 per-flag
stream); path + generic-args via
`<render_def_path(item)><render_generic_args(Ty::*.args, item)>`
per stage-12 D-12-6 + §6.7 (with D-14-4.9 `(args,
item)` signature); field types render via
`render_ty` per stage-13 D-13-5 (use-site path-only
form per §6.4, NOT recursive definition body --
preserves the use-site vs definition-site
architectural distinction).

**v1 deferrals (explicit out-of-scope):**
- `#[repr(...)]` decoration: not surfaced.
  `spec.md` §11.3 already drops the source `repr`
  annotation in favor of resolved layout bytes;
  reconstructing source-syntactic `#[repr(C)]` /
  `#[repr(packed)]` / `#[repr(int)]` from layout
  would require an inverse-derivation pass not
  needed for Phase-1 closure. Future revision per
  "Spec is mutable."
- Enum variant discriminant values (`Variant = N,`):
  not surfaced. Requires deferred `#[repr(...)]`
  decoration + signed-vs-unsigned dispatch.
- Field offsets / pack annotations: IR-internal
  layout details, not surfaced.
- Empty-struct-form disambiguation: three Rust
  source forms collapse to unit `struct Foo;`
  in rendered output.

**Spec contributions**: `pretty-printer-snapshot.md`
§10.2.4 new subsection "Struct variant rendering"
(three-case dispatch table + tuple + named-field
composition outlines + 6 worked examples covering
unit / tuple / generic-tuple / named-field /
generic-named-field-with-lang-attr / synthetic
closure-captures struct); §10.2.5 new subsection
"Enum variant rendering" (EnumShape dispatch table
+ per-variant ctor-form dispatch table + composition
outlines + 5 worked examples covering empty /
single / all-unit / mixed-ctor / generic enum);
§10.2.6 new subsection "Union variant rendering"
(named-field form + composition outline + 2 worked
examples simple + generic union). §10.2 intro
outline updated to mark §10.2.4 / §10.2.5 / §10.2.6
LOCKED. `spec-snapshot.md` §9.4 InstanceKind notes
augmented with a "Consumer-side rendering"
paragraph cross-referencing `pretty-printer.md`
§10.2 per-variant subsections (mirrors §9.7 Const
pointer added at D-14-4). §6.4 unchanged (use-site
path-only form coexists with definition-site
full-body form; different callers).

**Cluster status**: D-14-5 cluster Phase A drafting
in progress (5/8 sub-decisions LOCKED). Three
load-bearing locks done (D-14-5.1 API signature,
D-14-5.2 Function envelope, D-14-5.5 ADT body
choice). **Next concrete action**: lock D-14-5.6
(DomainAttr attribute weaving -- ordering rule for
the attribute block when both per-flag §5.3 stream
and `domain_attrs.item_attrs` stream contribute,
plus per-`AttrData` variant render rule -- stage-14
occupant `AttrData::Intrinsic(IntrinsicKind::Log(LogLevel))`
renders as `#[rseq::intrinsic("log-<sev>")]` with
dash-form NAME).

Previous: 2026-05-13. **D-14-5.3 + D-14-5.4
LOCKED at per-stage Phase A** -- Static and Const
variant rendering, the small surgical-add per-variant
locks composing D-14-4's `render_const` for the value
position. **D-14-5.3** (Static): form
`[<attrs>]static [mut ]<path>: <render_ty(ty)>[ = <render_const(init.unwrap(), ty)>];`.
Five aspects -- attribute block per D-14-5.2 contract
(stage-14 occupant `AttrData::Intrinsic` doesn't
target Static Items; block reduces to §5.3 per-flag
stream in v1); mutability prefix read from
`ItemKind::Static { mutability }` per stage-12
D-12-1.2 (c) (`Mutability::Not` -> `static `;
`Mutability::Mut` -> `static mut `); path via
`render_def_path` per stage-12 D-12-6 (bridge-
synthesised vtable instances + ConstMemoryStatic
carry their family-encoded paths unchanged); type
decoration `: <render_ty(ty)>` always rendered;
initializer-vs-declaration trailer dispatch on
`init: Option<ConstId>` -- `Some(c)` ->
` = <render_const(c, ty)>;` composing D-14-4.1
pair-form, `None` -> `;` (foreign extern static;
biconditional `init.is_none() iff
Item.linkage == Foreign` per §3.4 step 8 / D-12-2.2;
mirrors D-14-5.2's `body == None` declaration form).
**D-14-5.4** (Const): form
`[<attrs>]const <path>: <render_ty(ty)> = <render_const(value, ty)>;`.
Simpler than D-14-5.3 -- no mutability prefix (Rust
forbids `const mut`), no foreign-declaration form
(Rust forbids `extern { const FOO; }`), `value:
ConstId` always populated per stage-12 D-12-2 (no
Option wrap). Both sub-decisions surface the
**pre-evaluated** ConstId via `render_const` rather
than re-rendering the value-constructing MIR body
(rustc-asymmetry per the prior-art Q6 evidence:
RegseqModel has done the CTFE work; the renderer
exposes the evaluated form, not the evaluation
graph). **Spec contributions**: `pretty-printer-snapshot.md`
§10.2.2 new subsection "Static variant rendering"
with per-aspect tables (mutability table +
initializer-vs-declaration trailer table) +
composition outlines (definition + declaration
forms) + 5 worked examples (immutable static,
mutable static, foreign extern static,
ConstMemoryStatic synthetic, vtable-instance
synthetic); §10.2.3 new subsection "Const variant
rendering" with per-aspect token table + composition
outline + 6 worked examples (scalar i32, scalar
usize, struct const, enum-variant const, ZST
PhantomData const, reference-typed `&str` const).
§10.2 intro outline updated to mark §10.2.1 / §10.2.2
/ §10.2.3 LOCKED and clarify forward-pointers for
D-14-5.5 / D-14-5.6 / D-14-5.7. §5.4 / §6.5 / §8
unedited (final wording held for D-14-5.7 layering
lock; deferred). **Cluster status**: D-14-5 cluster
Phase A drafting in progress (4/8 sub-decisions
LOCKED). **Next concrete action**: lock D-14-5.5
(ADT variants Struct / Enum / Union -- load-bearing
architectural choice with no rustc precedent;
Option A header-only vs Option B full definition
body from `spec.md` §11 Layout; Option B is the
evidence preponderance per the prior-art Q7
evidence). D-14-5.5 has sub-questions for
`EnumShape` dispatch + per-variant ctor form +
`#[repr(...)]` decoration (likely deferred) +
discriminant value rendering (likely deferred).

Previous: 2026-05-13. **D-14-5.2 LOCKED at
per-stage Phase A** -- Function variant envelope, the
load-bearing per-variant lock. Five aspects pinned:
(1) attribute block placement (multi-line `#[...]`
prefix; composition rule -- ordering of per-flag §5.3
stream vs `domain_attrs.item_attrs` stream -- deferred
to D-14-5.6); (2) header-line keyword decoration
`[unsafe ][extern "ABI" ]fn <path>` -- `unsafe` from
`Item.is_unsafe` (matches §5.3), `extern "ABI"` from
`fn_sig.abi` (`FnAbi::Rust` default suppressed; only
non-default v1 form is `extern "C"`); (3) arg-list
grammar `(_1: T1, _2: T2, ..., _N: TN)` matching
`_<idx>` arg-local convention from §3.1 / §25.1, with
trailing `, ...` when `fn_sig.c_variadic == true` per
§14.3 invariant; (4) always-render `-> <ret>` form
(including unit case `-> ()`) for header-vs-locals
consistency with the §3.1 envelope's
`let mut _0: <ret>;` declaration; (5)
body-vs-declaration trailer dispatch on
`body: Option<Body>` -- `Some(b)` -> ` { <envelope> }`
per §3.1, `None` -> `;` (foreign-declaration form
surfacing emission units that rustc's MIR pretty-printer
skips per Q5 evidence). `flavor` / `virtual_idx` are
identity metadata (not rendering inputs); bridge-
synthesised family encoded in DefPath via stage-12
D-12-6 + §13.4. The body envelope (stage-13 D-13-2 §3.1)
is composed unchanged; D-14-5.2 wraps without redefining.
No D-13-2 REOPEN required.

Previous: 2026-05-13. **D-14-5.1 LOCKED at
per-stage Phase A** -- API signature
`render_instance(InstanceId) -> String` (single-arg
form; no pair-form, no `&Instance` borrow, no
per-variant multi-method dispatch trio). The
`PrettyPrinter` has `&model` and resolves `InstanceId`
internally to `&model.instances[id]`. Per-variant
dispatch is internal via a single `match inst.kind`.
Reasoning: (1) `InstanceKind` already embeds the
per-variant rich data each branch needs internally
(`fn_sig`/`body` for Function; `ty`/`init` for Static;
`ty`/`value` for Const; `layout` resolving to the
field/variant table for ADT variants) -- no consumer-
context type required at the call site; (2) mirrors
rustc_public `Instance::emit_mir(self, w)` precedent
at `rustc_public/src/mir/mono.rs:178`; (3) ID-form
consistency with the renderer family (`render_ty(TyId)`,
`render_const(ConstId, TyId)`, `render_def_path(ItemId)`,
`render_item(ItemId)` all take ID forms). `flavor` /
`virtual_idx` are identity metadata only -- the
bridge-synthesised flavor is encoded in the Item's
DefPath per stage-12 D-12-6 + §13.4, so the renderer
needs no per-flavor branch. **Spec contributions**:
`pretty-printer-snapshot.md` §10.2 intro replaced
with the full API contract + dispatch overview +
cross-references to building blocks; per-variant
subsections (§10.2.1 - §10.2.6) land as their
respective D-14-5.2 - D-14-5.5 sub-decisions lock,
no forward-looking placeholders per README §5.9
rule 4.

Previous: 2026-05-13. **D-14-5 cluster ALLOCATED
at per-stage entry-steps closure (README §5.1 steps
4-7).** Step 4 (rustc evidence pass) closed
(2026-05-13) with `prior-art/rustc-instance.md` (10
questions Q1-Q10) surveying rustc's body envelope
machinery (`rustc_middle/src/mir/pretty.rs`
`write_mir_pretty` / `write_mir_fn` / `write_mir_intro`
/ `write_mir_sig`), the per-MonoItem dispatch surface
(`rustc_codegen_ssa/src/mono_item.rs`), and the
stable-IR analog (`rustc_public/src/mir/mono.rs`
`Instance::emit_mir` + `rustc_public/src/mir/pretty.rs`
`function_body`). Three rustc asymmetries surfaced as
load-bearing: ADT variants (Struct/Enum/Union) have
no rustc precedent (Q7); foreign declarations
(`body: None`) need a declaration form that rustc
skips (Q5); Static/Const surface pre-evaluated
ConstIds via D-14-4 `render_const` rather than
value-constructing bodies (Q6). Step 5 allocated **8
sub-decisions** D-14-5.1 through D-14-5.8 (full
audit-shape entry in `discussions.md`):
**D-14-5.1** (load-bearing API signature -- single-arg
`render_instance(InstanceId) -> String`);
**D-14-5.2** (load-bearing Function variant envelope
-- attrs + `[unsafe ][extern "ABI" ]fn ...` signature +
body-vs-declaration dispatch on `body: Option<Body>`);
**D-14-5.3** (Static variant
`[<attrs>]static [mut ]PATH: ty[ = <render_const>];`);
**D-14-5.4** (Const variant
`[<attrs>]const PATH: ty = <render_const>;`);
**D-14-5.5** (load-bearing ADT variants, no rustc
precedent -- Option A header-only vs Option B full
body from §11 Layout; B is evidence preponderance);
**D-14-5.6** (DomainAttr weaving -- ordering rule +
per-`AttrData` render rule, stage-14 occupant
`AttrData::Intrinsic` -> `#[rseq::intrinsic("log-<sev>")]`);
**D-14-5.7** (load-bearing layering boundary --
composer-from-scratch (a) vs `render_item` wrapper (b);
recommendation (a)); **D-14-5.8** (cross-stage
friction watch; no anticipated REOPENs from current
evidence). Step 6 prior-art (Charon / mrustc /
rustc_public) deferred per D-14-4 precedent. Step 7
refined `memory.md` D-tag agenda + this file.

Earlier: 2026-05-13. **D-14-4 cluster LOCKED at
per-stage Phase B with R4 APPROVE / Lockability:yes /
Close-out:YES.** 4-round trajectory closed
(2026-05-13): R1 BLOCK (3B+1M+1N; operand Ty
threading not propagated through §7.3-§7.9 + cascade
incomplete + fat-pointer synthetic-TyIds violated
closed-Ty invariant + `render_generic_args` lacked
ItemId + stale "pending" wording) -> R2 BLOCK (1
R1.3 regression in status.md + 1 cross-stage friction
inconsistency MINOR + 1 obsolete callsite NIT) -> R3
BLOCK (1 R1.3 regression continuation + 1 §10.1.8
cascade-doc MINOR) -> R4 APPROVE (0 findings; 11
distinct findings closed total across the cluster).
The cluster expanded the D-13-6 REOPEN scope from
1 signature to **3 signatures**
(`render_operand(&Operand, &Body, expected_ty: TyId)`,
`render_rvalue(&Rvalue, &Body, expected_ty: TyId)`,
`render_generic_args(&GenericArgs, item: ItemId)`)
and added a small bridge-side invariant at
`spec-snapshot.md` §9.7 (auxiliary `Ty::Ptr(pointee,
Not)` + `Ty::Uint(Usize)` interned in `model.types`
when producing `Aggregate([Reloc, Uint])` for
fat-pointer consumer Tys). All 12 D-14-4
sub-decisions remain LOCKED at Phase A unchanged --
Phase B revisions were spec-text consistency, not
architectural reframings.

Earlier: 2026-05-13. **D-14-4 cluster LOCKED at
per-stage Phase A.** All 12 sub-decisions
(D-14-4.1 through D-14-4.12) drafted in a single
session covering the full
`render_const(ConstId, TyId) -> String` composer-
renderer surface across the closed 11-variant `Const`
enum. Final batch landed: **D-14-4.8** (Reloc per-arm
with Function / FnPtr-coercion / Static /
ConstMemoryStatic `b"..."` special case),
**D-14-4.4** (Aggregate per-arm: Array / Tuple /
Struct structural + `&str` / `&[u8]` fat-pointer
special cases), **D-14-4.5** (EnumValue per-arm:
`EnumShape::Single` / `Multiple` variant dispatch),
**D-14-4.6** (Bytes per-arm: all-uninit shortcut +
`*b"..."` byte-array case + `bytes![...]` structural
fallback), **D-14-4.9** (type-context threading +
operand-callsite wiring -- REOPENS stage-13 D-13-6
with a three-signature cascade per R1 finding:
`render_operand(&Operand, &Body, TyId)`,
`render_rvalue(&Rvalue, &Body, TyId)`, and
`render_generic_args(&GenericArgs, ItemId)`).
**Cross-stage friction**: D-13-6 REOPEN pending flip
at merger close with annotation listing all three
signatures. Spec-side edits landed at
`pretty-printer-snapshot.md` §10.1.3-§10.1.8 (stubs
replaced with full content), §7.2 / §7.3 / §6.7
(signature extensions), §7.4 / §7.6 / §7.7 / §7.9
(caller derivation columns), §6.4 (ADT callsites
threading `item`), §8 ownership table (all three rows
updated), §10.1 intro (operand-callsite + recursion
notes updated from "pending" to LOCKED references).
Also added `spec-snapshot.md` §9.7 "Bridge interning
invariant for fat-pointer auxiliary types" paragraph
(per R1.3 resolution: bridge interns
`Ty::Ptr(pointee, Not)` + `Ty::Uint(Usize)` in
`model.types` when producing
`Aggregate([Reloc, Uint])` for fat-pointer consumer
Tys; renderer trusts the invariant).
Phase B Codex review on the full
`pretty-printer-snapshot.md` D-14-4 contributions
pending; next concrete action is Phase A specimen
augmentation (~10-15 new specimens covering all
locked arms) then Phase B review.

Earlier: 2026-05-13. **D-14-4.7 LOCKED** at
per-stage Phase A (seventh sub-decision in D-14-4
cluster; second of the type-context-dependent
variants after D-14-4.3 ZST). Specifies the
rustc-mirrored typed-cast form `0x0 as
<render_ty(consumer_ty)>` for `Const::NullPtr(pointee_ty)`.
The consumer Ty from D-14-4.1's pair-form is
authoritative for the cast-type rendering (carries
full pointer-type structure: mutability + borrow-
flavor + FnPtr signature); the variant payload
`pointee_ty` is for canonical-equality only per §9.7.
Admissible consumer Tys are thin pointers only: 5
kinds (`Ty::Ref` shared / unique, `Ty::Ptr` const /
mut, `Ty::FnPtr`). Fat-pointer / non-pointer consumer
Tys cannot occur per the bridge's §9.7 dispatch
(single-scalar pointer layouts only); renderer
panics. Mirrors rustc's `pretty_print_const_scalar_int`
pointer-Ty arm at
`rustc_middle/src/ty/print/pretty.rs:1816-1826`. Spec
edit landed at `pretty-printer-snapshot.md` §10.1.6
(stub replaced with dispatch table + admissible-Ty
note + consistency invariant).

Earlier: 2026-05-13. **D-14-4.3 LOCKED** at
per-stage Phase A (sixth sub-decision in D-14-4
cluster; first of the type-context-dependent
variants). Specifies the consumer-Ty-driven dispatch
for `Const::ZST` across 7 categories: unit-tuple,
ZST-array (empty + N>0 elem-ZST), fieldless /
composite-ZST struct, single-variant enum (unit /
tuple / named-field variant), ZST union, FnDef,
non-capturing closure. Composite-ZST cases recurse on
the unique `Const::ZST` `ConstId` per §9.7 canonical-
form rule (single representative for all ZSTs), with
the per-sub-field `Ty` driving the form at each
recursion. Mirrors rustc's only ZST special-case
(`FnDef` -> value-namespace path) verbatim; produces
RegseqModel-richer rendering for the 6+ other ZST
consumer Tys where rustc falls back to debug. Cases
that cannot occur (`Ty::Never`, `EnumShape::Empty`,
`EnumShape::Multiple`, non-ZST scalar consumer Tys)
are rejected with a panic per *No defensive
fallbacks*. Tuple-vs-named-field distinction for
struct / variant fields follows the existing §3.4
rule (synthetic `_<index>` names = tuple-style).
Spec edit landed at `pretty-printer-snapshot.md`
§10.1.2 (replaced the stub with the full dispatch
table + recursion shape + cannot-occur cases).

Earlier: 2026-05-13. **D-14-4.12 LOCKED** at
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
  through D-14-6 + D-14-10 all LOCKED at Phase A +
  Phase B per `discussions.md`. Remaining D-tags from
  the pre-staged inventory (D-14-7 / D-14-8 / D-14-9)
  still open; additional D-tags may emerge per their
  cluster's evidence pass.
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

**D-14-6 cluster LOCKED at per-stage Phase B with
R6 APPROVE / Lockability:yes / Close-out:YES
(2026-05-13).** 6 sub-decisions LOCKED at Phase A
(D-14-6.1 through D-14-6.6) plus 6-round Phase B
Codex trajectory: R1 BLOCK -> R2-R5
MINOR/Lockability:yes recap-text-duplication cascade
-> R6 APPROVE (14 findings closed: 2 BLOCK + 12
MINOR + 0 NIT). The cluster covers the full top-level
`render(&self) -> String` composer-renderer surface
(iteration + separator + envelope + empty case) and
introduced **zero new cross-stage friction** --
D-14-6.6 zero-REOPEN claim held across the entire
trajectory. Full audit-shape in `discussions.md`
D-14-6 cluster entry; full per-round audit in
`review-log.md` D-14-6 cluster section.

**Stage 14 progress.** 6 of the 9 pre-staged D-tags
(plus the user-added D-14-10) are now LOCKED at
Phase B with R-level APPROVEs / close-outs:

| Cluster | Phase A LOCKED | Phase B verdict | Rounds | Findings |
|---------|----------------|------------------|--------|----------|
| D-14-1 + D-14-3 | 2026-05-12 | R3 MINOR / Close-out:YES | 3 | 8 (1B + 4M + 3N) |
| D-14-2 | 2026-05-13 | R6 APPROVE / Close-out:YES | 6 | 14 (0B + 11M + 3N) |
| D-14-10 | 2026-05-13 | R3 MINOR (Pass B) / Close-out:YES | 4 (multi-pass) | ~24 |
| D-14-4 | 2026-05-13 | R4 APPROVE / Close-out:YES | 4 | 11 (3B + 4M + 2N + 2 reg-cont) |
| D-14-5 | 2026-05-13 | R4 APPROVE / Close-out:YES | 4 | 13 (4B + 7M + 2N; 5 cont + 8 fresh) |
| D-14-6 | 2026-05-13 | **R6 APPROVE / Close-out:YES** | **6** | **14 (2B + 12M + 0N; recap-text-duplication cascade R2-R5)** |

The composer-renderer trio (D-14-4 `render_const` +
D-14-5 `render_instance` + D-14-6 top-level `render`)
is now fully specified per stage-13 D-13-4.6's
building-block-vs-composer architectural division.

**Active focus advances to D-14-7**
(`#[naked]` rejection in v1; external-review Finding
#2). D-14-7 reopens D-12-1.3 + D-9-9 + D-13-4.3 (the
`is_naked` Item field gets removed; `#[naked]`
rejection-table row added; per-flag rendering table
row dropped). After D-14-7 lock, the remaining
stage-14 clusters (D-14-8 forward-pointer retarget;
D-14-9 Specimen 70 split; `core::ffi::c_void`
rejection enforcement) are independent small surgical
fixes that can land in any order. See "Active
cluster" + "Next action" below for D-14-7 §5.1
entry-step plan.

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
- ~~**D-14-4.3**~~ -- **`ZST` per-arm** LOCKED
  2026-05-13. Consumer-Ty-driven dispatch across 7
  categories: unit-tuple `()`, ZST-array (empty `[]`
  + repeat `[<r>; N]`), fieldless / composite-ZST
  struct (path / tuple-form / named-field-form per
  §3.4), single-variant enum (unit / tuple-variant /
  named-field-variant), ZST union (path only), FnDef
  (path), non-capturing closure (path). Composite-ZST
  cases recurse on the unique `Const::ZST` `ConstId`
  per §9.7 canonical-form rule. Mirrors rustc's only
  `FnDef` special-case; RegseqModel-richer for other
  ZST Tys. Cannot-occur cases panic.
- ~~**D-14-4.4**~~ -- **`Aggregate` per-arm dispatch**
  LOCKED 2026-05-13. Dispatch on consumer `Ty.kind()`:
  Array `[c1, ...]`; Tuple `(c1, ...)` (n>1) / `(c1,)`
  (n==1); Struct unit / tuple-form / named-field per
  §3.4. Fat-pointer special cases: `Ty::Ref(_, Ty::Str)`
  -> `"<utf8-lossy>"`; `Ty::Ref(_, Ty::Slice(<u8>))`
  -> `b"<escape>"` (pre-conditions: 2-child Aggregate,
  first child is Reloc to ConstMemoryStatic synth
  Static with all-Value Bytes init, second is
  `Uint(Usize, matching len)`). Non-special fat-pointer
  fallback `(<r(c0, *const inner)>, <r(c1, usize)>) as <ty>`
  via D-14-4.9 bridge-interned auxiliary TyIds
  (`Ty::Ptr(pointee, Not)` + `Ty::Uint(Usize)` are
  bridge-interned in `model.types` per the §9.7 bridge
  interning invariant). Cannot-occur: Enum / Union /
  scalar / FnDef / Closure consumers -- panic.
- ~~**D-14-4.5**~~ -- **`EnumValue` per-arm** LOCKED
  2026-05-13. Dispatch on consumer `Ty::Enum`'s
  `EnumShape` (Single { variant: V } or Multiple {
  variants[variant_idx] }) to locate the named variant,
  then on `V.fields` for ctor form: unit variant ->
  `<enum_path>::<V.name>`; tuple variant ->
  `<enum_path>::<V.name>(c1, ...)`; named-field
  variant -> `<enum_path>::<V.name> { fields[0].name:
  c1, ... }`. Variant name via `EnumShape.Single.variant.name`
  or `EnumShape.Multiple.variants[idx].name`
  (non-optional `VariantLayout.name`). Cannot-occur:
  `EnumShape::Empty`, out-of-bounds `variant_idx`,
  payload arity mismatch, non-Enum consumer Tys --
  all panic.
- ~~**D-14-4.6**~~ -- **`Bytes` per-arm dispatch**
  LOCKED 2026-05-13. All-uninit shortcut `<uninit>`
  first (rustc-mirrored). Then `Ty::Array(<u8>, N)`
  with all-Value bytes -> `*b"<escape-ascii>"`
  (rustc deref form). Otherwise structural fallback
  `bytes![<byte_0>, ...]` with per-byte forms (`0xHH`
  for `Value(b)`; `<uninit>` for `Uninit`; `<reloc:
  <render_def_path>>` for `Reloc(_)` one token per
  maximal relocation run per §9.7 well-formedness).
  Cannot-occur: `&str` / `&[u8]` consumers (Aggregate
  path), scalar consumers -- bridge bugs, panic.
- ~~**D-14-4.7**~~ -- **`NullPtr(TyId)` per-arm**
  LOCKED 2026-05-13. `0x0 as <render_ty(consumer_ty)>`
  rustc-mirrored typed-cast form. Consumer Ty is
  authoritative for the cast type (not the variant
  payload `pointee_ty`, which is for canonical
  equality only per §9.7). 5 admissible thin-pointer
  consumer Ty kinds: `Ty::Ref` shared / unique,
  `Ty::Ptr` const / mut, `Ty::FnPtr`. Fat-pointer
  consumer Tys cannot occur (bridge produces
  `Aggregate([NullPtr, NullPtr])` instead);
  non-pointer consumer Tys are bridge bugs. Both
  panic per *No defensive fallbacks*.
- ~~**D-14-4.8**~~ -- **`Reloc(InstanceRef)` per-arm**
  LOCKED 2026-05-13. Resolves InstanceRef to
  InstanceId and dispatches on `(instance.kind,
  item_synthetic_kinds, consumer Ty)`. Function +
  `Ty::FnDef`: bare path (matches D-14-4.3 ZST-over-
  FnDef). Function + `Ty::FnPtr`: `<path> as <ty>`
  typed cast (coercion). Static (source / vtable /
  generic synth): `<path> as <ty>` typed cast.
  `ConstMemoryStatic` + `Ty::Ref(_, Ty::Array(<u8>,
  N), _)` consumer + all-Value Bytes init: special
  `b"<escape-ascii>"` (rustc-mirrored per
  `ty/print/pretty.rs:1727-1753`). Cannot-occur:
  `InstanceKind::Const / Struct / Enum / Union`;
  non-FnDef/FnPtr for Function; non-pointer for
  Static -- all panic.
- ~~**D-14-4.9**~~ -- **Type-context threading +
  operand-callsite wiring** LOCKED 2026-05-13.
  (a) Per-sub-field Ty resolution via structural
  layout walk: `Ty::Array` -> elem; `Ty::Tuple` ->
  fields[i]; `Ty::Struct` -> fields[i].ty; `Ty::Enum`
  -> V.fields[i].ty (Single/Multiple); fat-pointer
  `Ty::Ref(_, pointee, _)` -> bridge-interned
  auxiliary TyIds resolved via `TyInterner` lookup
  (data ptr = `Ty::Ptr(pointee, Mutability::Not)`,
  metadata = `Ty::Uint(UintKind::Usize)`; both
  guaranteed present in `model.types` per the §9.7
  bridge interning invariant added at R1.3).
  (b) **Stage-13 D-13-6 REOPEN with three-signature
  cascade** (per R1 finding):
  `render_operand(&Operand, &Body, expected_ty: TyId)`,
  `render_rvalue(&Rvalue, &Body, expected_ty: TyId)`,
  `render_generic_args(&GenericArgs, item: ItemId)`.
  `render_rvalue` extension forced because
  `Rvalue::Use(operand)` propagates consumer Ty to
  its operand; `render_generic_args` extension forced
  because `GenericArg::Const(c)` needs the enclosing
  Item's generic-parameter list. Caller cascade:
  `render_statement` Assign threads `place_ty` ->
  `render_rvalue(_, _, expected_ty)` ->
  `render_operand(_, _, derived_operand_ty)` ->
  `render_const(id, expected_ty)`. Copy/Move arms
  ignore `expected_ty`; only Constant consumes it.
  §6.4 / §6.7 / §7.2 / §7.3 / §7.4 / §7.6 / §7.7 /
  §7.9 caller specs + §8 ownership table (all three
  rows) updated; D-13-6 flip pending merger close.
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
4. **D-14-5** (`render_instance`) -- LOCKED 2026-05-13
   at Phase B R4 APPROVE.
5. **D-14-6** (top-level `render`) -- LOCKED
   2026-05-13 at Phase B R6 APPROVE.
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

- **D-14-4 cluster -- `render_const` composer renderer
  [LOCKED at Phase A / 2026-05-13]**. All 12
  sub-decisions LOCKED (see entries below + above).
  Spec-side edits across `pretty-printer-snapshot.md`
  §10 (new Composer renderers section with §10.1
  intro + §10.1.1 - §10.1.8 subsections), §7.2 /
  §7.3 / §6.7 (three-signature extensions per
  D-14-4.9 REOPEN of D-13-6), §7.4 / §7.6 / §7.7 /
  §7.9 (caller derivation columns for `expected_ty`),
  §6.4 (ADT callsites threading `item` to
  `render_generic_args`), §8 ownership table (all
  three method rows updated), §11 Cross-references
  (renumbered from §10), §9 specimen revisions to
  underscore-decimal form, §1.3 composer listing,
  §3.3 sigil convention. Also added
  `spec-snapshot.md` §9.7 "Bridge interning
  invariant for fat-pointer auxiliary types"
  paragraph (per R1.3). Cross-stage friction:
  **D-13-6 REOPENED** with three-signature cascade
  pending flip at merger close (annotation lists all
  three signatures per the cascade).

- **D-14-4.4 -- `Aggregate(Vec<ConstId>)` per-arm
  rendering [LOCKED at Phase A / 2026-05-13]**.
  Dispatch on consumer `Ty.kind()` across structural
  categories (Array / Tuple / Struct) + fat-pointer
  special cases (`&str` / `&[u8]`). Array -> `[c1, c2,
  ...]` (children count == N); Tuple -> `(c1, ...)`
  (n>1) or `(c1,)` (n==1); Struct unit / tuple-style
  / named-field per §3.4 distinction. Fat-pointer
  pre-conditions: 2 children with first being
  `Const::Reloc` resolving to a `ConstMemoryStatic`
  synth Static with all-Value Bytes init + second
  being `Const::Uint(Usize, len)` matching byte count.
  `&str` -> `"<utf8-lossy>"`; `&[u8]` -> `b"<escape>"`;
  fallback structural form `(<c1>, <c2>) as <ty>` via
  D-14-4.9 child-Ty resolution through bridge-interned
  auxiliary TyIds (`Ty::Ptr(pointee, Not)` +
  `Ty::Uint(Usize)` in `model.types` per the §9.7
  bridge interning invariant). Children count
  invariant bridge-validated. Cannot-occur:
  Enum / Union / scalar / FnDef / Closure consumer
  Tys panic.

- **D-14-4.5 -- `EnumValue { variant_idx, payload }`
  per-arm rendering [LOCKED at Phase A / 2026-05-13]**.
  Dispatch on consumer `Ty::Enum`'s `EnumShape` to
  locate the named variant. `EnumShape::Single { variant: V }`
  with empty `V.fields` -> `<enum_path>::<V.name>`
  (unit variant); non-empty `V.fields` -> tuple-form
  `<enum_path>::<V.name>(c1, ...)` or named-field-form
  `<enum_path>::<V.name> { fields[0].name: c1, ... }`
  per §3.4 distinction. `EnumShape::Multiple { variants }`
  with `V = variants[variant_idx]`: same three sub-forms.
  Variant name lookup via `EnumShape.Single.variant.name`
  or `EnumShape.Multiple.variants[idx].name`
  (non-optional `VariantLayout.name` per `spec.md`
  §11). `EnumShape::Empty` cannot occur (no values);
  `variant_idx` out-of-bounds + payload arity
  mismatch are bridge bugs; non-Enum consumer Tys
  panic.

- **D-14-4.6 -- `Bytes(Vec<Byte>)` per-arm rendering
  [LOCKED at Phase A / 2026-05-13]**. All-uninit
  shortcut `<uninit>` first (rustc-mirrored per
  `mir/pretty.rs:1864-1869`); then byte-array
  `*b"<escape-ascii>"` for consumer `Ty::Array(<u8>, N)`
  with all-Value bytes (rustc-mirrored per
  `mir/pretty.rs:1889-1898`); otherwise
  RegseqModel-richer structural fallback
  `bytes![<byte_0>, <byte_1>, ...]` with per-byte
  forms (`0xHH` lowercase 2-digit hex for `Value(b)`;
  `<uninit>` for `Uninit`; `<reloc:
  <render_def_path>>` for `Reloc(_)` one token per
  maximal relocation run per §9.7 well-formedness).
  Cannot-occur: `&str` / `&[u8]` consumer Tys
  (Aggregate path), scalar consumer Tys (scalar
  variants) -- bridge bugs, panic.

- **D-14-4.8 -- `Reloc(InstanceRef)` per-arm rendering
  [LOCKED at Phase A / 2026-05-13]**. Resolves
  InstanceRef to InstanceId (F4-2; bridge-validated)
  and dispatches on `(instance.kind,
  item_synthetic_kinds[instance.item], consumer Ty)`.
  `InstanceKind::Function` + `Ty::FnDef` consumer:
  bare path (matches D-14-4.3 ZST-over-FnDef --
  intentional; bridge may produce either
  `Const::ZST` or `Const::Reloc` for function-value
  references). `InstanceKind::Function` + `Ty::FnPtr`
  consumer: `<path> as <render_ty(consumer)>` (typed
  cast for FnDef -> FnPtr coercion).
  `InstanceKind::Static` (source-defined / vtable /
  generic synth): `<path> as <render_ty(consumer)>`
  typed cast. `ConstMemoryStatic` + `Ty::Ref(_,
  Ty::Array(<u8>, N), _)` consumer + all-Value Bytes
  init: special-case `b"<escape-ascii of init bytes>"`
  (rustc-mirrored per `ty/print/pretty.rs:1727-1753`).
  Cannot-occur: `InstanceKind::Const / Struct / Enum
  / Union`; non-FnDef/FnPtr consumer for Function-Reloc;
  non-pointer consumer for Static-Reloc; all panic.

- **D-14-4.9 -- Type-context threading +
  operand-callsite wiring [LOCKED at Phase A /
  2026-05-13]**. (a) Per-sub-field Ty resolution for
  recursive children: renderer walks parent consumer
  Ty's structural layout to extract per-child Tys.
  `Ty::Array(elem, N)` -> all children Ty = elem;
  `Ty::Tuple { fields }` -> i-th child Ty =
  fields[i]; `Ty::Struct { layout: Struct { fields } }`
  -> i-th child Ty = fields[i].ty; `Ty::Enum { ... }`
  with `EnumShape::Single { variant: V }` -> i-th
  payload child Ty = V.fields[i].ty; same with
  `EnumShape::Multiple` indexed by `variant_idx`;
  `Ty::Ref(_, pointee, _)` fat-pointer fallback ->
  per-child Tys resolved via `TyInterner` lookup
  (data ptr = `Ty::Ptr(pointee, Mutability::Not)`,
  metadata = `Ty::Uint(UintKind::Usize)`). **Bridge
  interning invariant** (per R1 finding, added at
  `spec-snapshot.md` §9.7 line ~3367): when the
  bridge produces `Const::Aggregate([Reloc, Uint])`
  for a fat-pointer consumer Ty, both auxiliary types
  are bridge-interned in `model.types`; the renderer
  trusts the invariant and looks up the existing
  TyIds at render time (preserves `spec.md` §10
  closed-Ty + all-interned-TyId invariant). (b)
  Operand-callsite wiring: **stage-13 D-13-6 REOPEN**
  with **three-signature cascade** (per R1 finding):
  (i) `render_operand(&Operand, &Body)` ->
  `render_operand(&Operand, &Body, expected_ty: TyId)`;
  (ii) `render_rvalue(&Rvalue, &Body)` ->
  `render_rvalue(&Rvalue, &Body, expected_ty: TyId)`;
  (iii) `render_generic_args(&GenericArgs)` ->
  `render_generic_args(&GenericArgs, item: ItemId)`.
  `render_rvalue` extension is forced because
  `Rvalue::Use(operand)` needs the consumer Ty to
  thread to its operand (the existing two-arg sig
  provides no source for that type).
  `render_generic_args` extension is forced because
  `GenericArg::Const(c)` needs the enclosing Item's
  generic-parameter list to resolve declared types.
  Caller cascade: `render_statement` Assign threads
  `place_ty` -> `render_rvalue(_, _, expected_ty)`
  -> `render_operand(_, _, derived_operand_ty)` ->
  `render_const(id, expected_ty)` per D-14-4.1
  pair-form. AssertKind operands use per-kind type
  rules (BoundsCheck both `Ty::Uint(Usize)`;
  Overflow uses BinOp input; MisalignedPointerDereference
  both `Ty::Uint(Usize)`). `Operand::Copy/Move(place)`
  arms ignore `expected_ty` (place type resolvable
  via `place_ty`); only `Operand::Constant(id)`
  consumes it. §6.4 / §6.7 / §7.2 / §7.3 / §7.4 /
  §7.6 / §7.7 / §7.9 caller specs updated; §8
  ownership table rows for all three methods updated;
  D-13-6 REOPEN flip pending merger close.

- **D-14-4.7 -- `NullPtr(TyId)` per-arm rendering
  [LOCKED at Phase A / 2026-05-13]**. Rustc-mirrored
  typed-cast form `0x0 as <render_ty(consumer_ty)>`.
  The variant payload `pointee_ty` is for canonical-
  equality only per `spec.md` §9.7
  (`NullPtr(t1) != NullPtr(t2)` whenever `t1 != t2`);
  the renderer uses the consumer Ty's full
  pointer-type rendering for the cast type (which
  carries mutability + borrow-flavor + FnPtr signature
  info the pointee-only payload does not).
  Admissible consumer Tys are thin pointers only --
  5 kinds: `Ty::Ref(T, SharedBorrow)` -> `0x0 as &T`;
  `Ty::Ref(T, UniqueBorrow)` -> `0x0 as &mut T`;
  `Ty::Ptr(T, Not)` -> `0x0 as *const T`;
  `Ty::Ptr(T, Mut)` -> `0x0 as *mut T`;
  `Ty::FnPtr(sig_id)` -> `0x0 as fn(...) -> ...`.
  Per the bridge §9.7 dispatch
  (`Scalar(Scalar::Int(_))` over a pointer type,
  all-zero -> `NullPtr(pointee_ty)`), the bridge
  produces `NullPtr` only for single-scalar
  (thin-pointer) layouts; fat-pointer null
  (`null::<dyn Trait>()`, `null::<[T]>()`,
  `null::<str>()`) is produced as
  `Aggregate([NullPtr, NullPtr])` or similar at
  D-14-4.4 territory. Consistency invariant: for an
  `Operand` / position holding `Const::NullPtr(pointee_ty)`,
  the consumer Ty is a thin pointer type whose
  pointee equals `pointee_ty`; bridge-validated at
  `spec.md` §3.3 Phase B step 9. Renderer trusts the
  invariant. Fat-pointer / non-pointer consumer Tys
  panic per *No defensive fallbacks*. Spec-side edit:
  `pretty-printer-snapshot.md` §10.1.6 stub replaced
  with full content. See `discussions.md` D-14-4.7
  for the audit trail; prior-art evidence at
  `rustc_middle/src/ty/print/pretty.rs:1816-1826`.

- **D-14-4.3 -- `ZST` per-arm rendering [LOCKED at
  Phase A / 2026-05-13]**. Consumer-Ty-driven
  dispatch for `Const::ZST` across 7 categories: (1)
  `Ty::Tuple { fields: [] }` -> `()`; (2)
  `Ty::Tuple { fields: [...] }` non-empty all-ZST ->
  `(<r(zst, T1)>, ...)` with recursion; (3)
  `Ty::Array(_, 0)` -> `[]`; (4) `Ty::Array(elem, N>0)`
  ZST -> `[<r(zst, elem)>; N]`; (5) `Ty::Struct {...}`
  fieldless / composite-ZST -> path, tuple-style, or
  named-field form per §3.4; (6) `Ty::Enum {...}`
  `EnumShape::Single` (unit / tuple-variant /
  named-field-variant) -> `<enum_path>::<variant_name>`
  with optional field form; (7) `Ty::Union {...}` ->
  path only (field-disambiguation NOT preserved for
  unions at the value level); (8) `Ty::FnDef(item,
  args)` -> `<render_def_path(item)><render_generic_args(args, item)>`
  (value-namespace path; mirrors rustc's only
  ZST-over-FnDef special-case at
  `rustc_middle/src/mir/pretty.rs:1976-1982`); (9)
  `Ty::Closure(item, args)` non-capturing -> same form.
  Composite-ZST recursion: the same `ConstId`
  representing the unique `Const::ZST` per §9.7
  canonical-form rule is threaded through recursive
  calls with per-sub-field consumer Ty driving the
  form at each step. Cannot-occur cases (`Ty::Never`,
  `EnumShape::Empty`, `EnumShape::Multiple`, non-ZST
  scalar consumer Tys) reject with panic. Tuple-vs-
  named-field distinction via §3.4 (synthetic
  `_<index>` names signal tuple-style).
  Spec-side edits: `pretty-printer-snapshot.md`
  §10.1.2 stub replaced with full dispatch table +
  recursion shape + cannot-occur cases. RegseqModel-
  richer than rustc's behaviour (rustc has only the
  FnDef special-case; falls back to debug for all
  other ZST Tys). See `discussions.md` D-14-4.3 for
  the audit trail.

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
- ~~**D-14-4**~~ -- LOCKED 2026-05-13 at Phase B R4
  APPROVE. 12 sub-decisions D-14-4.1 through
  D-14-4.12 all LOCKED. See "Locked decisions"
  section for full per-sub-decision breakdown and
  the "Active focus" + "Codex review trail" sections
  for the round trajectory.
- **D-14-5** -- LOCKED 2026-05-13 at Phase B R4
  APPROVE / Lockability:yes / Close-out:YES.
  `render_instance(InstanceId) -> String` composer
  renderer with 8 sub-decisions all LOCKED in a
  single Phase A drafting session (2026-05-13):
  D-14-5.1 (single-arg API signature, load-bearing);
  D-14-5.2 (Function variant envelope, load-bearing);
  D-14-5.3 (Static); D-14-5.4 (Const); D-14-5.5 (ADT
  variants Option B full-body, load-bearing, no
  rustc precedent); D-14-5.6 (two-stream attribute
  block composition); D-14-5.7 (composer-from-scratch
  layering with §5.6 shared helper, load-bearing);
  D-14-5.8 (zero new stage-13 REOPENs). Plus 19
  specimens (110-128) at `pretty-printer-snapshot.md`
  §9.11. Phase B closed at R4 APPROVE (4 rounds; 13
  distinct findings closed: 4B + 7M + 2N; 5
  continuations + 8 fresh -- recap-text-duplication
  pattern matching D-14-4). Full audit-shape entry
  in `discussions.md` D-14-5. Evidence basis at
  `prior-art/rustc-instance.md` (10 Q-tags). Spec
  contributions: `pretty-printer-snapshot.md` §1.3 +
  §3.3 + §5.6 + §8 + §10.2.1-§10.2.7 + §9.11 + §11;
  `spec-snapshot.md` §9.4 cross-reference paragraph.
- **D-14-6** -- Top-level `render(&self) -> String`
  composer renderer. Phase A LOCKED (6/6 sub-decisions)
  2026-05-13; Phase B LOCKED 2026-05-13 at R6 APPROVE
  / Lockability:yes / Close-out:YES (6-round
  trajectory: R1 BLOCK 6 -> R2-R5 MINOR/Lockability:yes
  cascade -> R6 APPROVE; 14 distinct findings closed
  across R1-R5).
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

**None currently in active drafting.** D-14-6
closed at Phase B R6 APPROVE / Lockability:yes /
Close-out:YES (2026-05-13). Spec contributions
landed at `pretty-printer-snapshot.md` §10.3
(§10.3.1-§10.3.7), §9.12 specimens 129-133,
§1.3 + §8 finalisations, `stage-14/prior-art/
rustc-render.md`, and `rustc-references.md`
Top-level render section.

D-14-6 was the **third and final composer-renderer
cluster** per stage-13 D-13-4.6. Closure unblocks
the remaining stage-14 work (D-14-7 / D-14-8 /
D-14-9 small surgical fixes + `core::ffi::c_void`
rejection enforcement + per-stage cross-cluster
Codex review on the full spec-snapshot.md +
pretty-printer-snapshot.md + merger close).

Per README §5.3, parallel sessions are not planned for
stage 14: D-tag clusters are not independent enough to
parallelize (composer renderers are sequential per the
D-14-4 -> D-14-5 -> D-14-6 dependency chain; Findings
are small enough to land alongside main decisions). Solo
work; no `sessions/` subfolder will be created.

## Cross-stage friction

D-14-2 + D-14-4 clusters surfaced **two** new
cross-stage friction points in addition to those
previously anticipated:

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

- **D-13-6 Body-level renderer per-arm specs --
  REOPEN.** D-14-4.9's operand-callsite wiring
  revises **three** stage-13 signatures (cascade
  expanded per R1 finding):
  - `render_operand(&Operand, &Body) -> String` ->
    `render_operand(&Operand, &Body, expected_ty:
    TyId) -> String` so that `Operand::Constant(id)`
    arms can thread the consumer-context Ty through
    to the pair-form `render_const(id, expected_ty)`
    per D-14-4.1.
  - `render_rvalue(&Rvalue, &Body) -> String` ->
    `render_rvalue(&Rvalue, &Body, expected_ty:
    TyId) -> String` because `Rvalue::Use(operand)`
    needs to propagate the consumer Ty to its
    operand, and the existing two-arg signature
    provided no source for that type.
  - `render_generic_args(&GenericArgs) -> String` ->
    `render_generic_args(&GenericArgs, item:
    ItemId) -> String` because `GenericArg::Const(c)`
    needs the enclosing Item's generic-parameter list
    to resolve the declared type per D-14-4.1's
    pair-form.
  The extensions propagate to all §7.3-§7.9
  caller-side specs (Rvalue / Statement /
  Terminator / AssertKind callers resolve
  `expected_ty` from their structural context) and to
  §6.4 ADT-rendering callsites (Struct / Enum / Union
  / Closure / FnDef pass adjacent `item`). Stage-13
  D-13-6's deliberate divergence from rustc's
  `Display for ConstOperand` (no `const ` prefix at
  the operand layer) is preserved. Flip in
  `decisions.md` at merger close with annotation
  "(revised in stage 14 D-14-4.9: `render_operand`
  and `render_rvalue` signatures extended with
  `expected_ty: TyId`; `render_generic_args`
  signature extended with `item: ItemId`)".

Locked friction points awaiting merger-close flip
(all three locked at D-14-7.9 / Finding #2; status
flips land in `decisions.md` at stage-14 merger close):

- **D-12-1.3 typed Item-level fields revision** (locked
  at D-14-7.9 / Finding #2). The `is_naked: bool` field
  is removed from the §8.0 Item schema; v1 rejects
  `#[naked]`-attributed Items at Item translation per
  the new §15.1 row. Status flip in `decisions.md` at
  merger close with annotation "(revised in stage 14
  D-14-7: `is_naked` field removed; v1 rejects
  `#[naked]`-attributed Items at Item translation.)".

- **D-9-9 rejection-vocabulary extension** (locked at
  D-14-7.9 / Finding #2). The §23.6.1 9-row terminator-
  translation table stays at 9 rows; the §15.1 rejection
  table grows by one Item-translation row for `#[naked]`,
  sibling to the §23.6.1 row 8 (`InlineAsm`). Status flip
  in `decisions.md` at merger close.

- **D-13-4.3 per-flag rendering table reduction**
  (locked at D-14-7.9 / Finding #2). Removing the
  `#[naked]` row shrinks the per-flag rendering table in
  `pretty-printer.md`. Status flip in `decisions.md` at
  merger close.

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

**D-14-4 cluster LOCKED at R4 APPROVE (2026-05-13).**
4 rounds total; 11 distinct findings closed (3 BLOCK +
4 MINOR + 2 NIT + 2 regression-continuations of
R1.3). Trajectory: R1 (BLOCK 3B+1M+1N) → R2 (BLOCK
1 R1.3-regression + 1M + 1N) → R3 (BLOCK 1 R1.3-
regression-continuation + 1M) → R4 APPROVE 0
findings. Compared to prior D-14-2 (6 rounds / 14
findings) and D-14-1+D-14-3 (3 rounds / 8 findings):
D-14-4 ran tighter than D-14-2 despite the larger
12-sub-decision surface, because the architectural
sub-decisions were sound and the rounds were
predominantly about cascade-completeness +
exact-phrase-sweep across recap text. The R1.3
finding (fat-pointer fallback synthetic-TyId vs
bridge-interning) was the load-bearing fix --
expanded the D-13-6 REOPEN from 1 signature
(render_operand) to 3 signatures (render_operand +
render_rvalue + render_generic_args) and added a
small bridge invariant at §9.7. The
regression-continuation pattern (R1.3 carrying
through R2 + R3) reflected recap-text duplication
across 4-5 status/spec locations that incremental
edits caught some-but-not-all of; R4 confirmed the
final sweep closed.

**D-14-5 cluster LOCKED at R4 APPROVE (2026-05-13).**
4 rounds total; 13 distinct findings closed
(4 BLOCK + 7 MINOR + 2 NIT; 5 R1/R2/R3
continuations + 8 fresh). Trajectory: R1 (BLOCK
3B+3M+1N: row-order recap divergence + hex-vs-
decimal Static specimens + arg-local-redeclaration
outline bug + declaration-trailer wording +
escaped-quotes input + stale forward-pointers +
unqualified "byte-identical" wording) → R2 (BLOCK
1B+2M+1N: §10.2 intro third-source-of-truth row-
order continuation + 7 stale "pending/deferred"
wordings + per-variant attribute-block cross-
reference gap at §10.2.4/5/6 + status.md prior-
entry continuation) → R3 (MINOR / Lockability:yes
0B+2M+0N: 5 more "defers/will pin/may/in progress"
wordings + §3.3 sigil row contradicting D-14-5.7
composer-from-scratch) → R4 APPROVE 0 findings.
Compared to D-14-4 (4 rounds / 11 findings),
D-14-5 ran similar-shape but slightly larger
finding count because of: (a) the §5.6 shared
helper factor-out added a new surface for recap
drift; (b) the row-order recap appeared at three
locations (§5.6 + D-14-5.6 audit + §10.2 intro)
rather than D-14-4's typical 4-5-spot recap
duplications; (c) the §3.3 sigil-table row was a
stage-13 D-13-4.6 forward-pointer that the
canonical-location sweep (R1+R2) missed and R3
caught. No load-bearing architectural decisions
were revised at Phase B; the 8 sub-decision rules
themselves all held. No stage-13 D-tag REOPENs
introduced.

**Anticipated upcoming clusters:** D-14-7 / D-14-8 /
D-14-9 small surgical fixes (~1-2 rounds each);
`core::ffi::c_void` rejection enforcement (no D-tag;
spec-text-only). Per-stage post-cluster
cross-validation review against full
spec-snapshot.md + pretty-printer-snapshot.md (~2-4
rounds at stage close). Post-merge tier-3 review
against top-level spec.md + pretty-printer.md per
D-13-9 / §5.5 step 6 amendment (~2-5 rounds at
merger close).

## Next action

**Stage 14 is CLOSED.** All in-stage work has
landed:

- **Phase-1 foundation extensions cluster (D-14-1 +
  D-14-2 + D-14-3)** -- LOCKED.
- **Generic-C backend codegen carve-out (D-14-10)**
  -- LOCKED.
- **Composer-renderer trio (D-14-4 + D-14-5 +
  D-14-6)** -- LOCKED.
- **External-review Finding surgical-fix clusters
  (D-14-7 + D-14-8 + D-14-9)** -- LOCKED.
- **`core::ffi::c_void` rejection enforcement
  cluster** -- LOCKED at Phase B R3 APPROVE
  2026-05-13.
- **Per-stage cross-cluster Codex review** --
  closed at R8 APPROVE / Lockability:yes /
  Close-out:YES (8 rounds; 12 findings closed,
  0B+12M+0N).
- **Merger close per README §5.5 7-step contract**
  -- landed 2026-05-13; the post-merge tier-3
  Codex review closed at R15 APPROVE /
  Lockability:yes / Close-out:YES (15 rounds; 19
  findings closed, 1B+18M+0N).

**Next major project step (lives at top-level
`status.md`, not here):** Phase-1 implementation
sweep #1 -- per README §2's "Spec before
implementation" amendment, the sweep implements
all phase-1 spec content (stages 1-14) against
the locked spec in one pass, extending the
compiler workspace (`crates/regseq-model` + the
bridge in `crates/regseq-driver` per Q1/Q4) +
generic-C backend (hosted userspace, libc,
`main()`, `printf`, host-platform integer
conventions) + DSL -> IR -> C -> captured-stdout
phase-1 test suite. After implementation sweep #1
closes, phase 2 opens with **Stage 15** (regseq-
trait scaffolding + Lifecycle + driver-struct
discovery + RegseqError) per the phase-2 queue
declared in README §1.3 + §2.

**This stage-14 folder is now historical.** The
canonical spec content lives in top-level
`spec.md` + `pretty-printer.md` +
`generic-c-backend.md`; the canonical decision
registry lives in top-level `decisions.md`; the
canonical audit trail lives in top-level
`discussions.md`; the canonical review trail
lives in top-level `review-log.md`. This folder's
`spec-snapshot.md` + `pretty-printer-snapshot.md`
+ `generic-c-backend-snapshot.md` are frozen with
"Snapshot at close of stage 14" headers.

**D-14-6 closed state recap.** Full Phase A + Phase B
audit-shape lives at `discussions.md` D-14-6 cluster
entry + `review-log.md` D-14-6 cluster section
(6-round trajectory: R1 BLOCK 6 -> R2-R5
MINOR/Lockability:yes cascade -> R6 APPROVE; 14
distinct findings closed; no stage-13 REOPENs). Spec
contributions landed at `pretty-printer-snapshot.md`
§10.3 (§10.3.1-§10.3.7) + §9.12 specimens 129-133 +
§1.3 / §8 finalisations + `stage-14/prior-art/rustc-
render.md` (6 Q-tags) + `rustc-references.md`
Top-level render section (4 file:line landmarks).

----

**Historical D-14-5 / D-14-6 cluster reference.** The
verbatim D-14-5 + D-14-6 Phase A audit-shape content
is intentionally NOT duplicated here. Read
`discussions.md` D-14-5 / D-14-6 entries for the full
Phase A audit-shape per sub-decision, and
`review-log.md` D-14-5 / D-14-6 sections for the full
Phase B round trails. Both 9-step Phase A plans
remain transferable as templates for any future
composer-renderer-style cluster, though stage-14's
composer trio is now complete.

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
