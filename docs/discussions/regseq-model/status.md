# Status

Last updated: 2026-05-13. **STAGE 14 CLOSED
(2026-05-13).** The 7-step merger contract (README
§5.5) has executed end-to-end:

- **Step 1 + 2**: `stage-14/discussions.md` +
  `stage-14/review-log.md` appended to top-level
  artifacts under `## Stage 14` headers.
- **Step 3**: All stage-14 D-tags (D-14-1 through
  D-14-10 + the no-D-tag `core::ffi::c_void`
  cluster) registered in `decisions.md` under a new
  `## Stage 14` section.
- **Step 4**: Five-decision REOPEN queue flipped in
  `decisions.md`:
  - **D-9-9** [REOPENED] (revised in stage 14
    D-14-7: `#[naked]` Item-translation row added
    at §15.1, sibling to §23.6.1 row 8).
  - **D-9-10** [REOPENED] (revised in stage 14
    D-14-2.5: B-4 and B-6 carved out for
    `IntrinsicKind::Log(_)` callees).
  - **D-12-1.3** [REOPENED] (revised in stage 14
    D-14-7: `is_naked` field removed).
  - **D-13-4.3** [REOPENED] (revised in stage 14
    D-14-7: `#[naked]` row removed; §5.6 row-order
    recap renumbered).
  - **D-13-6** [REOPENED] (revised in stage 14
    D-14-4.9: `render_operand` + `render_rvalue`
    signatures extended with `expected_ty: TyId`;
    `render_generic_args` extended with
    `item: ItemId`).
- **Step 5**: The three stage-14 snapshots lifted
  to top-level merger artifacts:
  - `stage-14/spec-snapshot.md` -> `spec.md`
    (13046 lines).
  - `stage-14/pretty-printer-snapshot.md` ->
    `pretty-printer.md` (4438 lines).
  - `stage-14/generic-c-backend-snapshot.md` ->
    `generic-c-backend.md` (new top-level file, 755
    lines, per D-14-10 carve-out + README §5.9
    rule 7).
- **Step 6**: Post-merge tier-3 Codex review on
  all four merger artifacts closed at R15 APPROVE /
  Lockability:yes / Close-out:YES after a 15-round
  trajectory (1B+18M+0N; 19 findings closed). The
  R1 BLOCK was a wrong terminator vocabulary in
  `generic-c-backend.md` §7; R2-R14 closed
  cross-reference accuracy, terminology hygiene,
  registry-vs-spec consistency, and co-equal-
  artifact framing drift. See top-level
  `review-log.md` `## Stage 14 / Post-merge tier-3
  review` for the round-by-round trail.
- **Step 7**: This status update + the three
  stage-14 snapshots are frozen with "Snapshot at
  close of stage 14" headers.

**Phase 1 spec closure status**: stage 14 was the
final phase-1 stage. All eleven stage-14 cluster
surfaces locked at Phase B APPROVE during stage-14
work + cross-cluster review + merger post-merge
review. **Phase 1 spec is now closed.** Eleven
clusters delivered:
1. **D-14-1 + D-14-3** -- Q7 intrinsic recognition
   + DomainAttr sibling-table architecture.
2. **D-14-2** -- Diagnostic-logging intrinsic
   vocabulary.
3. **D-14-10** -- Generic-C backend codegen
   carve-out (new co-equal merger artifact
   `generic-c-backend.md`).
4. **D-14-4** -- `render_const` composer renderer
   (12 sub-decisions).
5. **D-14-5** -- `render_instance` composer
   renderer (8 sub-decisions, 19 specimens).
6. **D-14-6** -- Top-level `render` composer
   (6 sub-decisions, 5 specimens).
7. **D-14-7** -- `#[naked]` rejection (Finding #2;
   9 sub-decisions; 3 REOPENs).
8. **D-14-8** -- Stale `emission_name` forward-
   pointer retarget (Finding #3).
9. **D-14-9** -- Specimen 70 split (Finding #4).
10. **`core::ffi::c_void` rejection enforcement
    cluster** (no D-tag; inherits stage-3 D16 +
    D1.4).

**Next major project step**: **Phase-1
implementation sweep #1** per README §2's "Spec
before implementation" amendment. The sweep
implements all phase-1 spec content (stages 1-14)
against the locked spec in one pass, extending the
compiler workspace (`crates/regseq-model` + the
bridge in `crates/regseq-driver` per Q1/Q4) +
generic-C backend (hosted userspace, libc,
`main()`, `printf`, host-platform integer
conventions) + DSL -> IR -> C -> captured-stdout
phase-1 test suite. After implementation sweep #1
closes, phase 2 opens with **Stage 15: regseq-
trait scaffolding + Lifecycle + driver-struct
discovery + RegseqError** per the phase-2 queue
declared in README §1.3 + §2 "Domain extensions
iterate per stage in phase 2".

Previous: 2026-05-13. **`core::ffi::c_void`
rejection enforcement cluster LOCKED at per-stage
Phase B R3 APPROVE / Lockability:yes / Close-out:YES
(2026-05-13).** 3-round Phase B trajectory closed
(2026-05-13): R1 MINOR 3 findings (0B+2M+1N;
Coroutine family double-classification in the new
§15.1 enforcement paragraph + stale line-number
citations at six audit + tracker sites + dangling
"Phase B." fragment at the audit closure check) ->
R2 MINOR 1 finding (0B+1M+0N; missed stale line
number at `stage-14/memory.md`) -> R3 APPROVE 0
findings. **Total: 4 findings closed across R1-R2
(0B+3M+1N; 3 rounds)**. All findings were spec-text
consistency at the audit + tracker + spec-
introduction text surfaces, not architectural
reframings. Lockability:yes sustained across R1-R3
(no BLOCK findings). All Phase A content unchanged
through Phase B; the structural directive (two new
§15.1 introduction paragraphs naming the §15.0 gate
+ defining the opaque-struct pattern) is intact.
**Phase 1 spec closure status**: this cluster closes
the **fourth and final stage-14 spec surface** (the
inherited stage-3 D16 + D1.4 expectation). **All
stage-14 clusters are now LOCKED at Phase B**:
- Foundation extensions (D-14-1 / D-14-2 / D-14-3)
- Generic-C backend carve-out (D-14-10)
- Composer-renderer trio (D-14-4 / D-14-5 / D-14-6)
- External-review Finding surgical fixes (D-14-7 /
  D-14-8 / D-14-9)
- `core::ffi::c_void` enforcement (no D-tag)

**No D-tag allocated** for this cluster -- stage-3
D16 already covers the decision-level commitment to
reject c_void in v1; stage-3 D1.4 already
recommends the opaque-struct pattern. **No D-tag
flip** -- stage-3 D16 and D1.4 are unchanged.
**Cross-stage friction**: none new. The five-
decision merger-time flip queue remains unchanged
at D-9-10 (D-14-2.5) + D-13-6 (D-14-4.9) + D-12-1.3
+ D-9-9 + D-13-4.3 (D-14-7.9). **Next concrete
action**: run the per-stage cross-cluster Codex
review on the full
`stage-14/spec-snapshot.md` +
`stage-14/pretty-printer-snapshot.md` per README
§5.5 step 6 + §5.6 Tier-2 discipline (anticipated
2-4 rounds; catches inter-cluster drift across the
entire stage-14 spec surface before merger). After
the cross-cluster review closes at APPROVE, hand
off to merger per the §5.5 7-step contract (flips
the five-decision queue to `[REOPENED]` in
`decisions.md`; integrates stage-14 snapshots into
top-level `spec.md` + `pretty-printer.md` +
`generic-c-backend.md`; freezes stage-14 snapshots
with "Snapshot at close of stage 14" headers;
updates this file to mark stage 14 closed and
announce phase-1 implementation sweep #1 as the
next major project step).
Pure spec hygiene cluster landing the enforcement
narrative at `stage-14/spec-snapshot.md` §15.1
introduction. Phase A drafting verified that the
c_void row already exists in the §15.1 rejection
table, the
§15.0 Special-ADT dispatch-ordering rule already
gates the rejection algorithmically, and Appendix
A.1 already walks the
`*mut c_void` -> `*mut OpaqueCtx` rewrite end-to-
end. **One applied edit**: two new paragraphs added
to the §15.1 introduction (after the four-category
narrative paragraph, before the rejection table) -
- an "**Enforcement model**" paragraph naming the
§15.0 dispatch-ordering gate as the algorithmic
enforcement for special-ADT rejections at the
"type translation" phase (enumerating which §15.1
rows the gate intercepts and noting that non-ADT
type-translation rejections + Item-attribute +
`PlaceElem` rows are enforced at their owning
sections' check points), and an "**Opaque-struct
pattern**" paragraph defining the pattern inline
(`#[repr(C)]` ZST struct with a single private
zero-length-array field; admitted via the ordinary
`Ty::Struct` path; lowers to incomplete-struct
pointer at the C-ABI boundary per §14.5 + §8.0
invariant 19; Appendix A.1 walks the end-to-end
rewrite). **No D-tag allocated** -- stage-3 D16
already covers the decision-level commitment to
reject c_void in v1; stage-3 D1.4 already
recommends the opaque-struct pattern. **No D-tag
flip** -- stage-3 D16 and D1.4 are unchanged. **No
sub-decisions allocated**. **Methodology rules
invoked**: README §5.9 rule 5 (no D-tag inline
references in spec text) + README §2 "C semantics
with information preservation" + README §2
"Forward expectations live in
`stage-N/expectations.md`". **Cross-stage
friction**: none new from this cluster. The five-
decision merger-time flip queue remains unchanged
at D-9-10 (D-14-2.5) + D-13-6 (D-14-4.9) + D-12-1.3
+ D-9-9 + D-13-4.3 (D-14-7.9). **Phase 1 spec
closure status**: this cluster closes the **fourth
and final stage-14 spec surface** (after D-14-7 /
Finding #2, D-14-8 / Finding #3, D-14-9 / Finding
#4 + the composer-renderer trio D-14-4 / D-14-5 /
D-14-6 + the foundation extensions D-14-1 / D-14-2
/ D-14-3 + the generic-C backend carve-out
D-14-10). After Phase B closes, the per-stage
cross-cluster Codex review on the full
`stage-14/spec-snapshot.md` +
`stage-14/pretty-printer-snapshot.md` runs
(anticipated 2-4 rounds), then hand-off to merger
per §5.5 7-step contract (flips the five-decision
queue to `[REOPENED]` in `decisions.md`; integrates
stage-14 snapshots into top-level `spec.md` +
`pretty-printer.md` + `generic-c-backend.md`;
freezes stage-14 snapshots with "Snapshot at close
of stage 14" headers; updates this file to mark
stage 14 closed and announce phase-1
implementation sweep #1 as the next major project
step). **Next concrete action**: kick off Phase B
R1 Codex close-out review verifying the new §15.1
introduction paragraphs land cleanly + the
references to §15.0 / §14.5 / §8.0 / Appendix A.1
all resolve to the current spec text. R1
anticipated at 0-2 findings APPROVE / Close-out:YES
(directive is single-section spec hygiene; no
sub-decisions; no cascade pattern expected --
matches D-14-8 / D-14-9 shortest-trajectory shape).

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
shape. **Phase 1 spec closure status**: D-14-9
closes the **third surgical-fix cluster** of stage
14 (external-review Finding #4; after D-14-7 /
Finding #2 and D-14-8 / Finding #3). Pure pretty-
printer-spec hygiene cluster landing the Finding
#4 split at `stage-14/pretty-printer-snapshot.md`
§9.9. **Two applied edits**: (i) Specimen 70 input
downshifts `opaque_handle_eligible: true` to
`false` and the output drops the `#[opaque_handle]`
line, leaving a clean Function attribute-stress
specimen; the D-14-7 cross-finding parenthetical
forward-pointing to D-14-9 is removed since the
fix has now landed. (ii) new Specimen 70b carrying
`kind: ItemKind::Struct, def_path: <pl011::Handle>,
opaque_handle_eligible: true` renders
`#[opaque_handle]` above `struct pl011::Handle`,
preserving coverage of the bridge-outcome rendering
on the only `ItemKind` that can carry the flag per
`spec-snapshot.md` §8.0 invariant 19. Specimen
count drifts ~75 -> ~76 via the 70b suffix;
gap-free 70 -> 70b -> 71 sequence acceptable per
D-13-7.2's policy-bucket-exception spirit. **No
sub-decisions allocated** -- the directive is
fully prescribed by `stage-14/expectations.md`
Finding #4. **No D-tag flip on D-13-7** -- D-13-7's
decision text is not revised. **Methodology rules
invoked**: README §5.9 rule 6 (`pretty-printer.md`
ownership of consumer-side rendering spec) +
D-13-7 + D-13-7.2 (specimen-coverage rubric +
policy-bucket-exception numbering) + §8.0
invariant 19 (Struct-only opaque-handle
eligibility) + README §2 "Forward expectations
live in `stage-N/expectations.md`". **Cross-stage
friction**: none new from D-14-9. The five-decision
merger-time flip queue remains unchanged at D-9-10
(D-14-2.5) + D-13-6 (D-14-4.9) + D-12-1.3 + D-9-9 +
D-13-4.3 (D-14-7.9). Active focus advances to the
remaining stage-14 work -- **`core::ffi::c_void`
rejection enforcement** (no new D-tag; stage-3 D16
already specifies). After that cluster locks, the
per-stage cross-cluster Codex review on the full
`spec-snapshot.md` + `pretty-printer-snapshot.md`
runs (anticipated 2-4 rounds), then hand-off to
merger per §5.5 7-step contract (flips D-9-10 +
D-13-6 + D-12-1.3 + D-9-9 + D-13-4.3 to
`[REOPENED]` in `decisions.md`).

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
drift; D-14-8's 2-round trajectory is the shortest
in stage 14 so far (matches Phase A trackers'
anticipation that the prescribed directive + one-
paragraph edit + no sub-decisions would close in
1-2 rounds). **Phase 1 spec closure status**:
D-14-8 closes the **second surgical-fix cluster**
of stage 14 (external-review Finding #3; after
D-14-7 / Finding #2). Pure spec hygiene cluster
landing the Finding #3 retarget at
`stage-14/spec-snapshot.md` §9.2 paragraph 3 (the
closing clause of the `Struct / Enum / Union`
`emission_name` bridge-construction-rule bullet,
now committing phase-1 to determinism + uniqueness
only and forwarding the full normative C-identifier
construction algorithm to the C-backend spec's
responsibility as out of phase-1 scope). **Cross-stage
friction**: none new from D-14-8.

Previous: 2026-05-13. **D-14-7 cluster LOCKED at
per-stage Phase B R6 APPROVE / Lockability:yes /
Close-out:YES (2026-05-13).** 6-round Phase B
trajectory closed (2026-05-13): R1 MINOR 3 findings
(0B+3M+0N; §8.0 field-count off-by-one + D-14-7
audit-section pre-lock header label + Q5 evidence
ALLOW_LIST `test`/`ignore`/`should_panic`
inconsistency) -> R2 MINOR 1 finding (0B+1M+0N;
recap-text-duplication tail across §3 status summary
+ memory.md) -> R3 MINOR 1 finding (0B+1M+0N; four
lagging surfaces at R1 trajectory) -> R4 MINOR 1
finding (0B+1M+0N; same surfaces shifted one round)
-> R5 MINOR 1 finding (0B+1M+0N; same shift forward;
**cascade-breaking fix applied** by bumping the four
surfaces to the full R5 trajectory at once including
R5 itself) -> R6 APPROVE 0 findings. **Total: 7
distinct findings closed across R1-R5
(0B+7M+0N)**. All 9 D-14-7 sub-decisions remain
LOCKED at Phase A unchanged -- Phase B revisions
were spec-text consistency (3 R1 findings + 4 rounds
of recap-text-duplication cascade), not
architectural reframings. Lockability:yes sustained
across R1-R6 (no BLOCK findings). **No new stage-13
/ stage-12 / stage-9 D-tag REOPENs introduced** by
any R1-R5 fix; the three D-14-7.9 REOPENs
(D-12-1.3, D-9-9, D-13-4.3) unchanged.
**Phase 1 spec closure status**: D-14-7 closes the
first surgical-fix cluster of stage 14 (external-
review Finding #2). The composer-renderer trio
(D-14-4 / D-14-5 / D-14-6) plus the first surgical
fix (D-14-7) are now fully specified. Spec
contributions: `spec-snapshot.md` §3.5.2 + §3.6.6 +
§8.0 + §15.1 edits; `pretty-printer-snapshot.md`
§5.1 + §5.2 + §5.3 + §5.6 + §9.9 edits;
`stage-14/prior-art/rustc-naked.md` (6 Q-tags) +
`rustc-references.md` "`#[naked]` handling" section
(5 file:line landmarks). **Cross-stage friction**:
three new REOPENs from D-14-7 (D-12-1.3, D-9-9,
D-13-4.3) join the pending merger-time flips from
prior clusters (D-9-10 from D-14-2.5, D-13-6 from
D-14-4.9). The five-decision merger-time flip queue
will land at stage-14 merger close per §5.5 step 4.

Previous: 2026-05-13. **D-14-7 cluster LOCKED at
per-stage Phase A (9/9 sub-decisions).** Phase A
drafting session (2026-05-13) closed the
external-review Finding #2 surface (`#[naked]`
rejection in v1) with 9 sub-decisions D-14-7.1
through D-14-7.9: **D-14-7.1 (load-bearing)** reject
at Item translation; diagnostic names `#[naked]` and
recommends future phase-2 stage-17
`#[rseq::interrupt(...)]` deferral path; **D-14-7.2**
recognition API
`tcx.codegen_fn_attrs(did).flags.contains(CodegenFnAttrFlags::NAKED)`
during stage-12 D-12-5 Phase 2 Translation+Synthesis;
**D-14-7.3** rejection-table placement at
`spec-snapshot.md` §15.1 with new "Item translation"
phase row + §15.1 header note extended to four
categories (Item-attribute DSL rejections joins
non-rigid `TyKind` / rigid DSL-rejected / closed
`PlaceElem` vocabulary rejections); **D-14-7.4 +
D-14-7.5** `Item.is_naked` field + `instance_is_naked`
helper deletions at §8.0 + §3.5.2 (14-helper
inventory downshifts to 13); **D-14-7.6**
pretty-printer per-flag table edits at §5.3 + §5.6
row-order recap + §5.1 atomic-shape example + §5.2
default-suppression narrative; **D-14-7.7** specimen
alignment at §9.9 (Specimens 69 + 70 + 75 input
shapes drop `is_naked`; Specimen 70 output drops
`#[naked]` rendered line; cross-finding parenthetical
references D-14-9 / Finding #4); **D-14-7.8** §3.6.6
paragraph 3 finalisation from pre-lock D-14-7-
territory wording to Stage-14-D-14-7 locked-state
wording; **D-14-7.9** cross-stage friction outcome:
three new REOPENs at merger close (D-12-1.3 + D-9-9
+ D-13-4.3) with the documented annotations per
`expectations.md` Finding #2. **Step 4 rustc
evidence pass landed** at
`stage-14/prior-art/rustc-naked.md` (6 Q-tags Q1-Q6
covering attribute parsing / source-of-truth flag /
body-shape contract / codegen-side dispatch /
attribute-target restrictions / structural overlap
with stage-9 D-9-9 row 8) +
`rustc-references.md` "`#[naked]` handling" section
(5 file:line landmarks). Spec contributions:
`spec-snapshot.md` §3.5.2 + §3.6.6 + §8.0 + §15.1
edits; `pretty-printer-snapshot.md` §5.1 + §5.2 +
§5.3 + §5.6 + §9.9 edits. **Cross-stage friction**:
three new REOPENs from D-14-7 (D-12-1.3, D-9-9,
D-13-4.3) join the pending merger-time flips from
prior clusters (D-9-10 from D-14-2.5, D-13-6 from
D-14-4.9). **Codex Phase B trajectory (in progress)**:
R1 MINOR / Lockability:yes / Close-out:NO 3 findings
(0B+3M+0N; §8.0 field-count off-by-one + D-14-7
audit section pre-lock header label + Q5 evidence
ALLOW_LIST inconsistency; all closed) -> R2 MINOR /
Lockability:yes / Close-out:NO 1 finding (0B+1M+0N;
recap-text-duplication tail -- §3 status summary +
`memory.md` cluster-status sentence; all closed) ->
R3 MINOR / Lockability:yes / Close-out:NO 1 finding
(0B+1M+0N; four lagging surfaces at R1 trajectory;
all closed) -> R4 MINOR / Lockability:yes /
Close-out:NO 1 finding (0B+1M+0N; same four
surfaces shifted one round -- R3 bumped to R2-
trajectory wording lagged canonical R3; all closed)
-> R5 MINOR / Lockability:yes / Close-out:NO 1
finding (0B+1M+0N; same shift forward -- R4 bumped
to R3-trajectory wording lagged canonical R4;
**cascade-breaking fix applied -- the four lagging
surfaces bumped to the full R5 trajectory at once
including R5 itself + "Phase B R6 close-out
pending" rather than stopping at R4**; all closed).
Cluster crossed Lockability:yes at R1 (no BLOCK
findings; architectural picture stable since R1).
**Phase 1 spec closure status**: D-14-7 closes the
first surgical-fix cluster (external-review Finding
#2). The remaining stage-14 work is two more
surgical-fix clusters (D-14-8 stale-forward-pointer
retarget per Finding #3; D-14-9 Specimen 70 split
per Finding #4) and `core::ffi::c_void` rejection
enforcement; after those clusters lock, the per-
stage cross-cluster Codex review on the full
`spec-snapshot.md` + `pretty-printer-snapshot.md`
runs (anticipated 2-4 rounds), then hand-off to
merger per §5.5 7-step contract. **Next concrete
action**: kick off D-14-7 Phase B R6 close-out
Codex review verifying R5 fixes land cleanly. R6
anticipated at 0 findings APPROVE / Close-out:YES
(the cascade-breaking fix synchronises the four
lagging surfaces with the canonical R5 trajectory;
D-14-6's R6 closed at zero findings under the same
pattern).

Previous: 2026-05-13. **D-14-6 cluster LOCKED at
per-stage Phase B R6 APPROVE / Lockability:yes /
Close-out:YES (2026-05-13).** 6-round Phase B
trajectory closed (2026-05-13): R1 BLOCK 6 findings
(2B+4M+0N) -> R2 MINOR / Lockability:yes 3 findings
(0B+3M+0N; recap-text-duplication cascade) -> R3
MINOR / Lockability:yes 1 finding (0B+1M+0N; residual
pre-lock planning text) -> R4 MINOR / Lockability:yes
2 findings (0B+2M+0N; §10.3.7 worked-example drift +
stale Next-action sentence) -> R5 MINOR /
Lockability:yes 2 findings (0B+2M+0N; audit
worked-example body envelope blank line + recap
surfaces lagging R4 trajectory) -> R6 APPROVE 0
findings. **Total: 14 distinct findings closed
across R1-R5 (2B+12M+0N).** All 6 D-14-6 sub-decisions
remain LOCKED at Phase A unchanged -- Phase B revisions
were spec-text consistency (recap-text-duplication
cascade), not architectural reframings. **No stage-13
D-tag REOPENs introduced** by any R1-R5 fix (D-14-6.6
zero-REOPEN claim holds across the entire trajectory).
**Phase 1 spec closure status**: D-14-6 closes the
third and final composer-renderer cluster per
stage-13 D-13-4.6. The composer trio (`render_const`
/ `render_instance` / `render`) is now fully
specified. Spec contributions:
`pretty-printer-snapshot.md` §10.3 (7 subsections
§10.3.1-§10.3.7) + §9.12 specimens 129-133 (5
specimens) + §1.3 + §8 finalisations +
`stage-14/prior-art/rustc-render.md` (6 Q-tags) +
`rustc-references.md` Top-level render section
(4 file:line landmarks).

Previous: 2026-05-13. **D-14-6 cluster Phase A
LOCKED (6/6 sub-decisions) + Phase B R5 MINOR /
Lockability:yes / Close-out:NO (2026-05-13).**
Top-level `render(&self) -> String` composer renderer
-- the third and final composer-renderer cluster per
stage-13 D-13-4.6 (`render_const` D-14-4 done;
`render_instance` D-14-5 done; top-level `render`
D-14-6 in Phase B). All 6 sub-decisions LOCKED at
Phase A: **D-14-6.1** API signature
`render(&self) -> String` zero-argument (no `single`
filter, no write-sink form); **D-14-6.2 (load-bearing)**
iteration order walking
`model.instances.iter_enumerated()` in canonical F4-2
order per `spec.md` §16.3 (no render-time re-sort;
F4-2 already groups by `regseq_instance_kind_rank`
per §9.3); **D-14-6.3** per-Instance separator --
top-level composer emits `"\n\n"` between consecutive
`render_instance(id)` outputs (which end at `}` or
`;` with no trailing newline per the §9.11 specimen
contract), no leading or trailing separator (first-flag
pattern adapted from rustc's `write_mir_pretty` at
`pretty.rs:328-335`); **D-14-6.4** file-level envelope
-- no header / no trailer / no metadata wrapper, output
starts at first rendered byte of first
`render_instance` output (Stream-1 + Stream-2 attribute
block per §10.2.7 when non-empty, otherwise the
kind-keyword line) and ends at last rendered byte (`}`
or `;`) with no trailing newline injected; **D-14-6.5**
empty-model case -- empty `String` `""` identity (loop
never runs, no separator emitted); **D-14-6.6**
cross-stage friction outcome -- zero new stage-13
D-tag REOPENs (the §9.11 specimens already established
the no-trailing-newline contract; D-14-5's
`render_instance` signature unchanged). **Codex Phase
B trajectory (in progress)**: R1 BLOCK 6 findings
(2B+4M+0N; all closed -- separator algorithm
re-architected to make composer own `"\n\n"` bytes;
envelope wording corrected to acknowledge attribute-
block first line vs keyword line; D-tag wording strip;
hex Uint -> decimal in worked examples; codegen
ordering qualification; const-items mis-classified as
empty-`mir_keys` example) -> R2 MINOR /
Lockability:yes / Close-out:NO 3 findings (0B+3M+0N;
recap-text-duplication cascade across D-14-6.1 sketch
+ D-14-6.3 audit narrative + prior-art Q3/Q4 reasoning
+ D-14-6.4 audit + cluster opening D-14-6.2
sub-decision allocation + stage-14/status.md +
top-level status.md) -> R3 MINOR / Lockability:yes /
Close-out:NO 1 finding (0B+1M+0N; residual pre-lock
planning text in `discussions.md` D-14-6 cluster
opening Question paragraph + `memory.md` D-14-6 agenda;
both rewritten to post-lock state) -> R4 MINOR /
Lockability:yes / Close-out:NO 2 findings (0B+2M+0N;
§10.3.7 worked-example output trailer claimed implicit
trailing `\n` contradicting §10.3.4 + missing
function-body blank line; `stage-14/status.md` Next
action trajectory sentence stale at R2) -> R5 MINOR /
Lockability:yes / Close-out:NO 2 findings (0B+2M+0N;
D-14-6.3 + D-14-6.4 audit worked examples in
`discussions.md` still missing function-body envelope
blank line that R4 added to canonical §10.3.7; three
recap surfaces lagged behind R4 trajectory).
**Cross-stage friction**: none new from D-14-6
R1+R2+R3+R4+R5 (D-14-6.6 zero-REOPEN claim holds). Pending merger-time flips unchanged: D-9-10
(D-14-2.5), D-13-6 (D-14-4.9), plus D-12-1.3 / D-9-9 /
D-13-4.3 (D-14-7 when locked). Spec contributions:
new `pretty-printer-snapshot.md` §10.3 (7 subsections
§10.3.1-§10.3.7) + §9.12 specimens 129-133 (5
specimens covering empty / single-Instance / multi-
Instance mixed rank / same-rank-group / full per-rank
spectrum) + §1.3 composer listing + §8 ownership-table
row finalisations + new `stage-14/prior-art/rustc-
render.md` (6 Q-tags Q1-Q6) + `rustc-references.md`
new Top-level render section (4 file:line landmarks).
Active focus advances to **D-14-6 Phase B R6 close-out
Codex review** -- verifying R5 fixes land cleanly +
final close-out check; on R6 APPROVE / Close-out:YES,
the cluster closes Phase B and D-14-6 finalises as
the final composer-renderer cluster. See
`stage-14/status.md` "Next action" for the detailed
R6 plan.

Previous: 2026-05-13. **D-14-5 cluster LOCKED at
per-stage Phase B R4 APPROVE / Lockability:yes /
Close-out:YES (2026-05-13).**
`render_instance(InstanceId) -> String` composer
renderer per-variant specs for the closed 6-variant
`InstanceKind` enum -- the second of stage 14's
three composer-renderer clusters per stage-13
D-13-4.6 (`render_const` D-14-4 done; `render_instance`
D-14-5 just done; top-level `render` D-14-6 next).
All 8 sub-decisions (D-14-5.1 through D-14-5.8)
LOCKED in a single Phase A drafting session
(2026-05-13): **D-14-5.1** (load-bearing single-arg
API `render_instance(InstanceId) -> String`;
`InstanceKind` embeds enough per-variant context so
no pair-form needed; mirrors rustc_public
`Instance::emit_mir(self, w)` precedent);
**D-14-5.2** (load-bearing Function variant envelope
across 5 aspects: attribute block via §10.2.7 +
keyword decoration `[unsafe ][extern "ABI" ]fn` +
arg-list grammar including `c_variadic` trailing
`, ...` + always-render `-> <ret>` + body-vs-
declaration trailer on `body: Option<Body>`);
**D-14-5.3** (Static variant
`[<attrs>]static [mut ]PATH: <ty>[ = <render_const(init, ty)>];`
with mutability from `ItemKind::Static`, `init=None`
foreign-declaration trailer per Q5 evidence);
**D-14-5.4** (Const variant
`[<attrs>]const PATH: <ty> = <render_const(value, ty)>;`
with `value` always populated per stage-12 D-12-2);
**D-14-5.5** (load-bearing ADT variants Struct /
Enum / Union -- Option B full-definition body from
§11 Layout chosen over Option A header-only per
prior-art Q7 evidence; three-case dispatch on
`(fields.len(), first-field-name-pattern per §3.4)`
+ `EnumShape` Empty / Single / Multiple dispatch +
per-variant ctor forms unit / tuple / named-field;
v1 deferrals: `#[repr(...)]` decoration not
rendered, enum variant discriminant values not
rendered, field offsets / pack not surfaced,
empty-struct-form three-way disambiguation
collapses to unit `struct Foo;`); **D-14-5.6**
(attribute block composition rule -- two-stream
interleaving: Stream 1 (Item-flag, §5.3 table-row
order via the §5.6 shared helper) precedes Stream 2
(`domain_attrs.item_attrs[item]` Vec-index order
per §3.6.2 D-14-3.3); per-`AttrData::Intrinsic`
render table for stage-14 vocabulary
(`Intrinsic(IntrinsicKind::Log(LogLevel))` ->
`#[rseq::intrinsic("log-<sev>")]` with dash-form
NAME); phase-2 stages append per-family render
rules); **D-14-5.7** (load-bearing layering
boundary -- composer-from-scratch locked:
`render_instance` does NOT call `render_item`;
shared internal helper `render_item_attribute_block(item:
ItemId) -> String` factored from §5.3 / §5.5 keeps
both renderers' Stream 1 attribute streams
byte-identical without API coupling);
**D-14-5.8** (cross-stage friction outcome --
**zero new stage-13 D-tag REOPENs from D-14-5
drafting**; helper factor-out is additive spec
text within §5, not a table modification; pending
merger-time flips from prior clusters unchanged).
**Codex Phase B review closed at R4 APPROVE with
Lockability:yes / Close-out:YES (4 rounds; 13
distinct findings closed: 4 BLOCK + 7 MINOR + 2
NIT; 5 R1/R2/R3 continuations + 8 fresh -- the
recap-text-duplication pattern matching D-14-4 with
audit-narrative + sigil-table + forward-pointer
spots surfacing across rounds)**. **Cross-stage
friction**: none new from D-14-5; pending merger-
time flips unchanged: D-9-10 (D-14-2.5), D-13-6
(D-14-4.9), plus D-12-1.3 / D-9-9 / D-13-4.3 (when
D-14-7 locks). Spec contributions:
`pretty-printer-snapshot.md` §1.3 composer listing
finalised + §3.3 sigil row finalised + §5.6 new
shared helper subsection + §8 ownership table rows
finalised (`render_instance`, `render_item`, new
`render_item_attribute_block` row) + §10.2 intro +
§10.2.1-§10.2.7 per-variant + attribute-composition
subsections + §11 cross-reference retargeted +
§9.11 specimens 110-128 (19 specimens);
`spec-snapshot.md` §9.4 Consumer-side-rendering
cross-reference paragraph (mirrors §9.7 Const
pointer added at D-14-4). Active focus advances to
**D-14-6** (`render(&self) -> String` top-level
composer renderer; iterates `model.instances` in
Q2 codegen-keyed order, composes per-Instance via
the now-locked `render_instance`) -- the third and
final composer-renderer cluster per stage-13
D-13-4.6. See `stage-14/status.md` "Next action"
for the detailed D-14-6 scope.

Previous: 2026-05-13. **D-14-4 cluster LOCKED at
per-stage Phase B (2026-05-13).** `render_const(ConstId,
TyId) -> String` composer renderer per-arm specs for the
closed 11-variant `Const` enum -- the first of stage 14's
three composer-renderer clusters per stage-13 D-13-4.6.
All 12 sub-decisions (D-14-4.1 through D-14-4.12)
LOCKED in a single Phase A drafting session:
**D-14-4.1** (load-bearing pair-form API signature
`render_const(ConstId, TyId) -> String`; consumer-context
TyId required for type-context-dependent arms; mirrors
rustc's `pretty_print_const_value_tcx(tcx, ct, ty, fmt)`
at `rustc_middle/src/mir/pretty.rs:1851`); **D-14-4.2**
(scalar variants Bool / Char / Int / Uint / Float with
rustc-mirrored underscore-decimal convention --
`42_i32`, `42_u32`, `1.5f32`, `'a'` -- + decimal /
char / finite-float precision rules); **D-14-4.10**
(Int / Uint sentinel collapsing: 18-entry
rustc-mirrored sentinel set -- `i8::MIN` through
`isize::MAX` for signed; `u8::MAX` through
`usize::MAX` for unsigned; `Isize`/`Usize`
target-pointer-width-independent); **D-14-4.11**
(integer hex form admission: decimal-only for v1;
future-extension boundary recorded but not
pre-allocated); **D-14-4.12** (Float non-finite
rustc-mirrored underscore-broken form `inf_f<width>`
/ `-inf_f<width>` / `NaN_f<width>`; NaN bit-pattern
collapsing; negative zero is finite); **D-14-4.3**
(ZST per-arm exhaustive consumer-Ty dispatch across 7
categories of ZST Ty; composite-ZST cases recurse on
the unique `Const::ZST` canonical-form ConstId per
§9.7; cannot-occur cases panic; **RegseqModel-richer
than rustc** for most ZST consumer Tys where rustc
falls back to debug); **D-14-4.7** (NullPtr per-arm
rustc-mirrored typed-cast form `0x0 as
<render_ty(consumer_ty)>` across 5 admissible
thin-pointer consumer Ty kinds; fat-pointer consumer
Tys cannot occur per the bridge's single-scalar
dispatch); **D-14-4.8** (Reloc per-arm dispatch on
resolved Instance kind + `item_synthetic_kinds`;
Function `Ty::FnDef` bare path / Function
`Ty::FnPtr` typed-cast / Static typed-cast /
ConstMemoryStatic `b"..."` byte-string special case
rustc-mirrored); **D-14-4.4** (Aggregate per-arm
consumer-Ty dispatch on Array / Tuple / Struct
structural + `&str` / `&[u8]` fat-pointer special
cases per `mir/pretty.rs:1874-1888`); **D-14-4.5**
(EnumValue per-arm `EnumShape::Single`/`Multiple`
variant dispatch with unit / tuple / named-field
ctor forms per §3.4); **D-14-4.6** (Bytes per-arm
all-uninit shortcut + `*b"..."` byte-array case +
**RegseqModel-richer structural fallback**
`bytes![<byte_0>, ...]` with per-byte three-state
forms); **D-14-4.9 (load-bearing): operand-callsite
wiring with three-signature D-13-6 REOPEN cascade**
-- `render_operand(&Operand, &Body) -> String` ->
`render_operand(&Operand, &Body, expected_ty: TyId)`,
`render_rvalue(&Rvalue, &Body) -> String` ->
`render_rvalue(&Rvalue, &Body, expected_ty: TyId)`,
`render_generic_args(&GenericArgs) -> String` ->
`render_generic_args(&GenericArgs, item: ItemId)` --
plus a small bridge-side invariant added at
`spec-snapshot.md` §9.7 (auxiliary `Ty::Ptr(pointee,
Not)` + `Ty::Uint(Usize)` interned in `model.types`
when producing `Aggregate([Reloc, Uint])` for
fat-pointer consumer Tys per `spec.md` §10's
closed-Ty + all-interned-TyId invariant).
Spec contributions: new
`pretty-printer-snapshot.md` §10 Composer renderers
(§10.1 `render_const` intro + §10.1.1 - §10.1.8
subsections), §7.2 / §7.3 / §6.7 signature extensions
+ §7.4 / §7.6 / §7.7 / §7.9 caller derivation
columns + §6.4 ADT callsite updates + §8 ownership
table updates for all three revised signatures + §11
Cross-references (renumbered from §10) + new §9.10
specimens (34 specimens 76-109 covering all locked
arms) + §9 specimen revisions to underscore-decimal
form; `spec-snapshot.md` §9.7 bridge-interning
invariant paragraph addition; `rustc-references.md`
new "Const rendering (MIR pretty-printer)" section
with 8 file:line landmarks. **Codex Phase B review
closed at R4 APPROVE with Lockability:yes /
Close-out:YES (4 rounds; 11 distinct findings closed:
3 BLOCK + 4 MINOR + 2 NIT + 2 regression-continuations
of R1.3 the longest-tail finding)**. **Cross-stage
friction**: **D-13-6 REOPENED** by D-14-4.9
three-signature cascade (annotation "revised in
stage 14 D-14-4.9: `render_operand` and
`render_rvalue` signatures extended with
`expected_ty: TyId`; `render_generic_args` signature
extended with `item: ItemId`"); D-9-10 REOPEN
unchanged from D-14-2.5. Active focus advances to
**D-14-5** (`render_instance(InstanceId) -> String`
composer renderer) -- the second of the three
composer-renderer clusters per D-13-4.6. See
`stage-14/status.md` "Next action" for the detailed
D-14-5 scope.

Previous: 2026-05-13. **D-14-10 cluster LOCKED at
per-stage Phase B (Pass B R3 MINOR; 2026-05-13).**
Generic-C backend codegen carve-out -- relocates the
phase-1 generic-C backend lowering rules (previously
mixed into `spec-snapshot.md`) into a new dedicated
companion file `generic-c-backend-snapshot.md`,
keeping the IR-side spec focused on the producer
surface. 7 sub-decisions (D-14-10.1 through D-14-10.7
per `stage-14/discussions.md`) cover the relocation
contract. Cross-stage friction: none. Inbound
expectations filed to `stage-15` (Linux logging
binding), `stage-16` (MMIO worked-example +
generic-C backend MMIO lowering), `stage-17`
(atomic-acquire worked-example + generic-C backend
atomic lowering).

Previous: 2026-05-13. **D-14-2 cluster LOCKED at
per-stage Phase B (2026-05-13).** Diagnostic-logging
vocabulary + bridge contract -- the sole phase-1 domain
extension carve-out -- is now closed at the spec side.
9 sub-decisions (D-14-2.1 through D-14-2.9) cover the
full end-to-end surface: severity vocabulary + dash-form
intrinsic NAME mapping (D-14-2.1); intrinsic function
shape `fn log_<sev>(fmt: &'static str, args:
&[LogArg<'_>])` (D-14-2.2); closed 10-variant `LogArg`
carrier with From-impl-closure rule (D-14-2.3);
declarative `macro_rules!` contract with canonical
expansion shape (D-14-2.4); **load-bearing bridge
translation contract** -- Option A bridge desugars to
flat-arg Call; `I-DiagLog-CallArity` named invariant;
DiagLogPeel pass in Phase 5 body materialisation (third
body-construction-time pass alongside Drop-as-Call);
transient fingerprint discipline (D-14-2.5; **REOPENS
D-9-10** per §23.6.2 B-4 + B-6 carve-outs); restricted
format-template grammar with closed leading modifier
subset + trailing-newline rule (D-14-2.6); bridge-side
validation D1-D8 with D5 deferred to stage 17 and D8
target-aware `max_str_args_per_call` (D-14-2.7);
deferrals to stages 15 / 17 + future revisions +
explicit out-of-scope items (D-14-2.8); per-target
backend lowering contract -- generic-C `[ERROR] fmt\n`
binding + Linux `pr_*` family with deferred
`max_str_args_per_call` declaration + `&str`
null-termination normative contract with 8-arg cap; 4
informational targets (Zephyr / FreeBSD / Windows /
macOS) recorded (D-14-2.9). Adopted the v2 candidate
from `regseq-taxonomy-work/.../03-diagnostic-logging/
refined-construct.md` as the source-of-truth DSL
surface. Spec contributions: §3.6.3 (NAME mapping) +
§23.6.2 B-4/B-6 exception clauses + §25.7 three-pass
framing + new §25.7.3 DiagLogPeel + renumbered §25.7.4
Pass scheduling + new §26 (11 subsections covering
author surface end-to-end). **Codex Phase B review
closed at R6 APPROVE with Lockability:yes /
Close-out:YES (6 rounds; 14 distinct findings closed:
0 BLOCK + 11 MINOR + 3 NIT)**. **Cross-stage friction**:
D-9-10 [REOPEN] pending flip at merger close
(annotation "revised in stage 14 D-14-2.5"). D-13-6
verified NOT-REOPENED. **Inbound expectations filed**:
`stage-15/expectations.md` (`register_log_module` glue
+ device-context joining + Linux
`max_str_args_per_call`); `stage-17/expectations.md`
(call-site atomic-context fact for §26.8 D5). Active
focus advances to **D-14-4** (`render_const` composer
renderer); pretty-printer-snapshot.md edits begin
here. See `stage-14/status.md` "Next action" for the
detailed D-14-4 scope.

Previous: 2026-05-12. **Phase methodology revised**:
README §1.3 now describes 3 phases (domain-agnostic
foundation -> iterative domain extensions -> device
pressure); README §2 adds "Dual backend" + "Domain
extensions iterate per stage in phase 2" methodology
rules and amends "Spec before implementation" to reflect
the new implementation cadence (implementation sweep #1
after stage-14 close, then incremental implementation
passes after each phase-2 stage's close). **Stage 14 plan
narrowed**: the original monolithic "Stage 14:
Domain-specific extensions" is replaced by **Stage 14:
Phase-1 foundation extensions** -- Q7 intrinsic-recognition
mechanism (per external-review Finding #1) + closed
diagnostic-logging intrinsic vocabulary + DomainAttr
sibling-table architecture on `RegseqModel` + 3 composer
renderers (`render_const`, `render_instance`, top-level
`render`) per stage-13 D-13-4.6 + external-review
Findings #1-#4 closure. The per-domain-family work that
the original stage-14 plan monolithically owned moves
to phase-2 stages 15, 16, ... allocated lazily.
**Tentative phase-2 queue**: Stage 15 = regseq-trait
scaffolding + `Lifecycle` + driver-struct discovery +
DT-binding metadata + `RegseqError`; Stage 16 = MMIO
family; Stage 17 = interrupt / ISR family; Stage 18 =
atomic family; Stage 19+ = clock / reset, register-bank,
... TBD by device-pressure findings. **Dual backend
architecture** committed: generic-C backend (hosted
userspace, libc, `main()`, `printf`) closes phase 1
alongside stage-14 implementation sweep; Linux kernel
backend (`printk`, `module_init` / `module_exit`,
kernel-only APIs) extends incrementally with each
phase-2 stage. **Stage-14/expectations.md split**: the
foundation items stay in `stage-14/expectations.md`;
per-domain-family items relocate to the appropriate
phase-2 stage's expectations file as those stages open
lazily (follow-up edit after this status update lands).

**Follow-up (2026-05-12 same day): Stage 14 opened +
D-14-1 + D-14-3 cluster LOCKED at per-stage Phase A.**
Stage-14 folder fully populated (`expectations.md`
trimmed, `spec-snapshot.md` + `pretty-printer-snapshot.md`
copied from top-level, `memory.md` + `status.md` drafted,
`discussions.md` + `review-log.md` recording the cluster
work, `prior-art/rustc.md` from the Step-4 rustc evidence
pass). Phase-2 stage folders 15 / 16 / 17 / 18 created
with their inbound `expectations.md` bundles. README §1.3
(three-phase methodology) + §2 ("Dual backend" + "Domain
extensions iterate per stage in phase 2") amendments
landed. **D-14-1** (Q7 intrinsic recognition mechanism +
storage with 7 sub-decisions; recognition codepath
`tcx.get_attrs_by_path(did, &[sym::rseq, sym::intrinsic])`
verified against rustc-src; `register_tool` validation
delegated to rustc; bridge-side malformed-payload
validation; `IntrinsicKind::Log(LogLevel)` category +
sub-discriminant shape; two-phase population timing
Phase-2-recognition / Phase-4-materialization mirroring
§3.4 `vtables` pattern). **D-14-3** (DomainAttr
sibling-table architecture with 7 sub-decisions; unified
`DomainAttrTables` with discriminated `AttrData` enum;
three sibling tables `item_attrs` / `field_attrs` /
`span_attrs` -- `instance_attrs` dropped per stage-12
D-12-1.8 pull-through pattern; `Vec<AttrData>`
Phase-2-encounter-order preserved through Phase-4
materialization; validator-enforced per-family duplicate
invariants; attribute-namespace storage rule
[`#[rseq::*]` -> DomainAttr; recognized Rust attrs ->
typed Item fields per D-12-1.3]; per-family target-table
mapping). Codex Phase B review closed at R3 MINOR with
Lockability:yes / Close-out:YES (3 rounds; 8 findings
closed: 1 BLOCK + 4 MINOR + 3 NIT -- tighter than every
prior stage's Phase B trajectory). Active focus advances
to D-14-2 (diagnostic-logging vocabulary). See
`stage-14/status.md` "Next action" for the detailed
D-14-2 scope.

Previous: 2026-05-07 (**stage 13 closed and merged**:
all 8 main D-tags + 41 sub-decisions LOCKED + 1 D-tag
closed-by-absorption (D-13-8 → D-13-9.6) across a
single combined cluster. **D-13-1** (Master rendering
style + method shape: Rust-source-style with sigil
decorations + `PrettyPrinter` methods returning
`String`; 4-peer prior-art convergence on rustc-internal,
rustc_public, Charon, project's own
`render_def_path_value` precedent; mrustc dual-rendering
rejected as anti-pattern; **D14 closes by absorption**
into D-13-1.3); **D-13-2** (Body envelope +
indentation + sigil placement: rustc-converged
`fn $path(_1: T, ...) -> Ret { let _N: T; ...; bbN: { ... } }`
shape; 4-space INDENT, 8-space stmt indent; inline
sigil wirings `_<id.index()>` / `bb<id.index()>` at
first consumer; `unsafe`/attribute-prefixed function
header in D-13-4 territory); **D-13-3** (Span-rendering
mode + debuginfo gating: default-off, opt-in via
`PrettyPrintOptions { include_spans: bool,
include_debug_names: bool }` constructed via
`PrettyPrinter::with_options`; existing `new(model)`
preserved; trailing inline `// <render_span>` aligned
to col 40 when on; standalone `debug NAME => _N;`
lines after locals when on; synthetic-span
suppression for `SourceFileId::SYNTHESIZED`);
**D-13-4** (Item-level rendering shape: Rust-source
`#[...]` attribute syntax; default-suppression for
all default/false-valued fields; per-flag rendering
table source-faithful where rustc has the attribute;
per-ItemKind kind-keyword + DefPath form; multi-line
when ≥1 attribute renders, one-line otherwise;
**D-13-4.6 building-block-vs-composer architectural
division** -- stage 13 owns NEW building blocks
[`render_ty`, `render_item`, body-level renderers],
inherited helpers [`render_def_path` stage 12 per
D-12-6, `render_def_path_value` + `render_span`
stage 1] consumed but not stage-13-owned, stage 14
owns COMPOSERS [`render_const` per D-4-K,
`render_instance`, top-level `render`]); **D-13-5**
(`render_ty` per-arm specs across the closed
19-variant `Ty` enum; `Ty::Dynamic` =
`dyn <vtable-DefPath> + AutoTrait + AutoTrait`;
`render_generic_args` helper); **D-13-6** (Body-level
renderer per-arm specs: 9 public renderers + 1
internal helper across the closed PlaceElem /
Operand / Rvalue / Statement / TerminatorKind /
BasicBlock vocabularies); **D-13-7** (Worked-example
specimens for snapshot stability: ~75 specimens
covering ≥1 per public renderer + ≥1 per
closed-enum-variant at per-arm spec level with
policy-bucket exception; inline in pretty-printer.md
§9; 1:1 mirrored to insta snapshots at end-of-phase-1
implementation sweep); **D-13-8** [CLOSED-BY-ABSORPTION]
(§18.1 sigil-table review + §18.2 ownership-table
consolidation absorbed into D-13-9 §3 + §8);
**D-13-9 the load-bearing emergent decision**
(Pretty-printer spec location: full pretty-printer
spec moves to top-level companion file
`pretty-printer.md`; `spec.md` §18 becomes
one-paragraph pointer; deferred-renderer rows in §15.x
/ §19.8 / §20.6 / §21.8 / §22.8 / §23.9 / §24.9
drop and replace with one-line "Rendering: see
`pretty-printer.md` §<N>" pointers; methodology rule
extension to README §5.9 rule 6 + §5.5 step 5/6/7
amendments + §4 folder layout addition; matches
project's existing `architecture.md` /
`decisions.md` / `regseq-background.md` /
`rustc-references.md` companion-file pattern). **Most
load-bearing finding: D-13-9 emergent reframe**
prompted by user mid-cluster question "could we
create a separate pretty-printer.md instead of adding
to spec.md?" -- promoted to a new main D-tag,
absorbing D-13-8's table consolidations into the new
file structure. **Cross-stage closure** flagged at
merger contract close: stage-3 D14 flipped to
`[CLOSED-BY-ABSORPTION]` in `decisions.md` with
annotation `(absorbed into D-13-1 per stage 13)`.
Total stage-13 Codex cost: **8 rounds** (4 per-stage
Phase B + 4 post-merge tier-3); **39 distinct findings
closed** (8 BLOCK + 28 MINOR + 4 NIT-equivalent across
trajectories). Stage-13 lands tighter than stage-9 (12
rounds / 51 findings), stage-12 (10 rounds / 31), and
stage-11 (11 rounds / 26). NEW top-level merger
artifact `pretty-printer.md` (1532 lines) created at
stage-13 merger close per D-13-9; `spec.md` shrunk
from 11955 to 11580 lines (375 lines pruned via
D-13-9.2 cross-reference replacements). README §4 +
"Top-level files" paragraph + §5.5 steps 5/6/7 +
§5.9 "Six rules" + workflow + freeze rule all
extended per D-13-9.3.

Previous: 2026-05-06 (**stage 12 closed and merged**:
all 7 top-level D-tags + 29 sub-decisions LOCKED across
a single combined cluster. D-12-1 (Item schema + ItemKind
shape with **Option B reframe** as the load-bearing
finding -- restores symmetry across all 6 InstanceKind
variants by moving `fn_sig` + `body` from Item to
`InstanceKind::Function`, reopening stage-11 D-11-3 +
D-11-5; mutability moves from `InstanceKind::Static` to
`ItemKind::Static { mutability }` per stage-1 intent;
Item-level typed fields `linkage: Linkage` (4-variant)
+ `link_section: Option<StringId>` + `inline_hint:
InlineHint` (4-variant) + `is_unsafe: bool` + `is_naked:
bool` per stage-1 R5.2; DomainAttr forward-pointer to
stage-14 sibling-table architecture per stage-4
§1287-1299; hybrid `as_function() ->
Option<&FunctionInstanceData>` accessor + match arms;
13-helper Instance-side pull-through method inventory
on `RegseqModel`); D-12-2 (foreign / extern dispatch:
`body: None ⟺ tcx.is_foreign_item(did)` for Function;
`init: None ⟺ tcx.is_foreign_item(did)` for Static;
4-variant Linkage closed enum); D-12-3 (lang-item
recognition: closed 3-flag set per §8.2 carries; AdtFlags
predicate-based recognition; non-flagged lang-items
queried on-demand at consumer sites; reject opaque
LangItem tag and Charon-style string-based field per
§17.4 closed-vocabulary rule); D-12-4 (construction-
invariant closure for stage-1 deferrals 3 / 4 / 6;
`BridgeInput { spec_crate_stable_id }` parameter on
validate; landing site Phase B step 3 (a)..(g));
D-12-5 (canonical 5-phase bridge construction-pipeline
order: Discovery -> Translation+Synthesis -> F4 Sort ->
ItemId Assignment + side-tables -> Instance Construction
+ Phase 5 body work [materialisation -> Drop-as-Call ->
vtable rewrite -> validation]; new §3.4 subsection;
synthesis interleaved during Phase 2 with family-specific
Phase-2 working caches keyed by stable handles; post-sort
ItemId assignment; cross-reference handling during
construction is bridge-implementation choice); D-12-6
(`render_def_path(ItemId)` stub fill; one-line cache
lookup body; coexists with stage-1's
`render_def_path_value(&DefPath)`); D-12-K (categorical
Item-level rendering deferral to stage 13, compositional
with D-11-K cascade; outbound 7-entry bundle to
`stage-13/expectations.md` "From stage 12 (per D-12-K)").
**Most load-bearing finding: D-12-1.4 Option B reframe**
of stage-11 D-11-3 + D-11-5 -- the contradiction between
the rest of the spec's pre-mono Item model (§16.1 F4
sort + §13.4 source DefPathHash + §9.3 RegseqInstanceKey)
and stage-3 §15.1's rejection of Param/Bound TyKind
variants forced restoring symmetry by moving fn_sig +
body to `InstanceKind::Function`. **Cross-stage
reopenings** flagged at merger contract close: stage-11
D-11-3 + D-11-5 flipped to `[REOPENED]` in `decisions.md`
with annotation `(revised in stage 12 per D-12-1.4)`.
Total stage-12 Codex cost: 10 rounds (7 per-stage +
3 post-merge); 31 distinct findings closed (20 BLOCK +
11 MINOR + 0 NIT). Stage-12 lands within the stage-9 /
stage-10 / stage-11 trajectory band. Outbound bundles
filed: `stage-13/expectations.md` "From stage 12 (per
D-12-K)" (7-entry Item-level rendering bundle:
render_item, ItemKind variant rendering, Linkage / InlineHint
enum rendering, Function-flag rendering, link_section
rendering, lang-item flag rendering, opaque_handle_eligible
rendering); `stage-14/expectations.md` "From stage 12"
(Q7 intrinsic recognition extension + DomainAttr sibling-
table architecture confirmation; carry-over from
stage-1 / stage-4). Cross-stage finding F-12-Reify
(rustc `InstanceKind::ReifyShim` payload drift to
`(DefId, Option<ReifyReason>)` since stage-9; Instance-
level concern; flagged for stage-9 next review cycle if
codegen / phase-2 device pressure surfaces collisions).

Previous: 2026-05-05 (**stage 11 closed and merged**:
all 8 top-level D-tags + 1 sub-decision (D-11-6.1) +
D-11-K LOCKED across a single combined cluster. D-11-1
(Body 4-field flat shape `{ basic_blocks, local_decls,
arg_count, span }`; ~14 rustc Body fields rejected per
the §25.1 rejection table); D-11-2 (LocalDecl 4-field
shape `{ ty, mutability, span, debug_name: Option<StringId> }`
with stage-8 D-8-3's named-local lift from `var_debug_info`);
**D-11-3 the load-bearing reframe** (FnSig location:
Item-side, NOT on Body; supersedes stage-1 inbound's
Body-side framing per "Spec is mutable"; 4-peer
convergence on rustc / rustc_public / Charon / mrustc;
stage-12 absorbs `Item.kind = Function { fn_sig: FnSigId,
... }` per §14.3 5-field schema preserved); D-11-4
(LocalId via `newtype_index!` + `const RETURN_PLACE = 0`
using stage-10 D-10-3.2 const-body macro arm); D-11-5
(no BodyId; bodies as `Item.body: Option<Body>`
structural field; 4-peer convergence); D-11-6 (joint
validation visitor at `crates/regseq-model/src/visit.rs`
with 10 visit/super hook pairs; absorbs 5 deferred walks
from D-5-2 / D-7-3 / D-8-4 / D-9-10 / D-10-5; rustc_public
`make_mir_visitor!` template); **D-11-6.1** (construction
invariants 1+9 fold into visitor hooks; no separate
pre-pass); **D-11-7 hybrid (c) timing** (Drop-as-Call at
bridge construction time -- forced by stage-9 D-9-1.1's
Drop-variant rejection; vtable-indirect rewrite as
post-construction pass -- Charon
`transform_dyn_trait_calls` + mrustc `MIR_Cleanup_Virtualize`
peer precedents; cross-stage friction documentation with
stage-5 D-5-1 lacking `PtrMetadata` PlaceElem -- forces
multi-statement rewrite shape with 4-5 fresh locals;
pass scheduling: rewrite precedes joint validation
walk); D-11-8 (single push API on Body --
`Body::push_local(decl) -> LocalId` + convenience helper
`Body::push_synthetic_local(ty) -> LocalId`; no
BodyBuilder; no buffering); **D-11-K** (categorical
rendering deferral retargeting D-5-K / D-6-K / D-7-K /
D-8-K / D-9-K / D-10-K + stage-1 _N sigil + stage-10
bbN sigil to stage 13 as a coherent rendering-decision
surface). **Most load-bearing finding: D-11-3 reframe**
of stage-1 inbound's "FnSig as Body field" via 4-peer
evidence convergence on Item-side placement; "Spec is
mutable" methodology rule applied. **D-9-3 reopening
question** raised mid-Phase-A (alt (a) Direct(InstanceId)
+ codegen-side dispatch on flavor:Virtual would have
removed stage-11 vtable-rewrite pass) concluded
"don't reopen" -- the cross-stage friction with stage-5
D-5-1 forces multi-statement shape regardless; stage-9's
IR-uniformity preference is reasonable; reopening cost
is unfavorable. Total stage-11 Codex cost: 11 rounds
(6 per-stage + 5 post-merge); 26 distinct findings
closed (9 BLOCK + 17 MINOR + 0 NIT). Stage-11 lands
between stage-9 (12 rounds / 51 findings) and stage-10
(9 rounds / 29 findings) -- consistent with the larger
spec content (10 D-tags + cross-stage reframes).
Outbound bundles: `stage-12/expectations.md`
"From stage 11" (FnSig migration + Body materialisation +
invariant 9 closure); `stage-13/expectations.md`
"From stage 11 (per D-11-K)" (12-item rendering bundle
with predecessor-stage attribution).

Previous: 2026-05-05 stage 10 closed and merged:
all 6 top-level D-tags + 21 sub-decisions LOCKED across
a single combined cluster. D-10-1 (2-field `BasicBlock {
statements: Box<[Statement]>, terminator: Terminator }`;
drops `is_cleanup` panic=abort discharge from stage-9
D-9-9 + `after_last_stmt_debuginfos` debuginfo discharge
from stage-8 D-8-3; non-Optional terminator type-level
guarantee; D-10-1.1-1.6 per-aspect sub-decisions);
D-10-2 (bare `IndexVec<BlockId, BasicBlock>` storage; no
wrapper / no CFG cache / no alias; **principled
model-vs-analysis line**: model stores facts the bridge
owns or rustc has resolved, not analyses derivable via
standard CFG/SSA algorithms; the single carve-out is
RPO ordering as the storage layout itself); D-10-3
(`BlockId` typed-index via `newtype_index!` + macro
const-body arm extension + size/niche assertions + `bbN`
sigil convention; closes 4 stage-1 inbound expectations);
D-10-4 (B-RPO storage order + B-Entry `START_BLOCK =
BlockId(0)`; both inherited from `ReorderBasicBlocks` +
START_BLOCK fixed-point; walk deferred to stage 11);
D-10-5 (5-part invariants registry: 3-row rejection
table bridge-enforced + 8 positive invariants
[B-0a/B-0b inherited + B-RPO/B-Reachable/B-Entry/
B-Terminator-Some/B-Successors-Valid/B-NonEmpty
introduced]; walk-impl deferred to stage 11; F4/F4-2
OUT; Part E forward-looking only); D-10-K
(render_basic_block deferred to stage 11; closes the
D-5-K through D-9-K cascade; 5-item outbound bundle
filed in `stage-11/expectations.md`). **Most
load-bearing finding: post-merge R1 surfaced the
missing `Terminator.successors()` definition** -- §24.4.3
referenced this method but stage-9 had never formally
defined it (no consumer before stage 10). Resolution:
added §23.1.4 as a stage-10-driven refinement of
stage-9 content per README §5.9 ("later stage refines
earlier stage's content"); per-variant successor
enumeration table cited against rustc's
`terminator.rs:527-568`. Per-stage R1-R2 also surfaced
the RPO-vs-unreachable inconsistency (resolution:
**B-Reachable** promoted from non-invariant to Part B
positive invariant; verified by stage-11 DFS-RPO walk;
rustc-pipeline scheduling informational, not
load-bearing) and walk-impl vacuity (resolution:
concrete DFS-RPO procedure with B-NonEmpty +
B-Successors-Valid pre-checks). Total stage-10 Codex
cost: 9 rounds (5 per-stage + 4 post-merge); 29
distinct findings closed (4 BLOCK + 16 MINOR + 9 NIT).
Tighter than stage-9's 12-round / 51-finding trajectory
because stage-10's surface is structural (per-block
container + body-level table) rather than a
big-vocabulary enum. Five-item outbound bundle filed in
`stage-11/expectations.md` "From stage 10":
(i) `render_basic_block` deferral; (ii) Body walk
implementation per §24.6.3 + §24.4.3 joint DFS-RPO
procedure; (iii) `BlockId` sigil-rendering helper
introduced alongside first consumer; (iv) Body-level
basic-block table integration (`Body.basic_blocks:
IndexVec<BlockId, BasicBlock>`); (v)
`LocalId::RETURN_PLACE` migration to the new
`newtype_index!` const-body arm.

Previous: 2026-05-05 stage 9 closed and merged: all 11
D-tags + 5 sub-decisions LOCKED across a single
combined cluster. D-9-1.1 (cardinality 6: Goto,
SwitchInt, Return, Unreachable, Call, Assert);
D-9-1.2-1.6 per-variant payloads; D-9-2 (Drop-as-Call
collapse at the bridge with concrete-vs-dyn-Trait drop
split per rustc `InstanceKind::Virtual(_, 0)` for
dyn-drops); **D-9-3 the load-bearing decision** locked
at candidate (b) `CallCallee::{Direct(InstanceId),
Indirect(Operand)}`, triggering a §9.4 schema extension
that added `virtual_idx: Option<u32>` to
`InstanceKind::Function`; D-9-4 drop per-arg spans
entirely (rustc_public-aligned; supersedes D-6-1.3);
D-9-5 8-variant AssertKind subset (drop 3 coroutine-only
variants); D-9-6 SwitchTargets explicit-split storage;
D-9-7 inline Terminator storage; D-9-8 wrapper
`{ kind, span }`; D-9-9 9-row rejection table (6
pipeline-discharged + 2 DSL-only + 1 collapsed);
D-9-10 4-part invariants registry with 11 positive
invariants (B-0a/c inherited + B-1-B-9 introduced;
B-9 added at post-merge R5 for target/callee-output
coherence); D-9-K render_terminator + 4 sub-renderers
deferred to stage 11. Total stage-9 Codex cost: 12
rounds (6 per-stage + 6 post-merge); 51 distinct
findings closed (15 BLOCK + 30 MINOR + 6 NIT).

Previous: 2026-05-05 stage 8 closed and merged: all
six D-tags LOCKED across a single combined cluster.
D-8-1 (3-variant closed `StatementKind` surface --
`Assign(Place, Rvalue)`, `SetDiscriminant(Place,
VariantIdx)`, `Intrinsic(NonDivergingIntrinsic)` -- with
4 sub-decisions D-8-1.1-1.4 covering variant cardinality
and per-variant payload shapes) LOCKED; D-8-2 (no
Statement interning) LOCKED; D-8-3 (`Statement { kind,
span: Span }` wrapper, drop scope and debuginfos,
named-local preservation outbound to stage 11) LOCKED;
D-8-4 (5-part registry: 12-row rejection table covering
10 distinct dropped rustc variants + 8 positive
invariants B-0a/B-0b/B-0c inherited + B-1-B-5 stage-8-
introduced; walk-enforcement deferred to stage 11)
LOCKED; D-8-K (`render_statement` deferred to stage 11
alongside `render_place`, `render_operand`,
`render_rvalue`) LOCKED. Merger contract per README §5.5
completed: top-level `discussions.md`, `review-log.md`,
`decisions.md` extended with stage-8 entries; stage-8
spec contributions integrated into top-level `spec.md`
(new §22 Statement vocabulary section + §18.2 per-method
ownership row update for `render_statement`); post-merge
Codex review pass R1 closed at MINOR with `Lockability:
yes` on 2026-05-05 (5 findings closed: 0 BLOCK + 4 MINOR
+ 1 NIT). Total stage-8 Codex cost: 5 rounds (4
per-stage + 1 post-merge); 20 distinct findings closed
(3 BLOCK + 16 MINOR + 1 NIT). Notable substantive
trajectory: per-stage R1 BLOCK caught the
`Ty::Adt(NominalId)` umbrella fabrication (stage-3 has
direct `Ty::Struct/Enum/Union` per §10, no `Ty::Adt`
umbrella; `NominalKind` exists as a `find_nominal`
key, not a `Ty` discriminator); per-stage R2 BLOCK
narrowed B-5 from rustc's full `builtin_deref(true)`
admissible set to RegseqModel-representable subset
(`Ty::Ptr` and `Ty::Ref`; `Box<T>` rejected at stage-3
§15.1 cannot reach stage 8); post-merge R1 corrected
the `SetDiscriminant` codegen claim (writes
`VariantLayout.tag_value: Option<u128>` per §11.1, not
a `VariantIdx`-indexed table). Three-item outbound
expectation to file in `stage-11/expectations.md`
"From stage 8" (`render_statement` deferral; Statement
walk implementation per D-8-4 Part C; named-local
preservation per D-8-3, `LocalDecl.debug_name`
populated from rustc's body-level `var_debug_info`).

Top-level project tracker. Captures cross-stage signals only:
high-level stage progress, active focus, blockers, and deferred
items affecting multiple stages. Stage-specific progress lives in
`stage-N/status.md`.

For project entry point and invariants, read `README.md` first.

## Phase 1 -- domain-agnostic foundation

Spec stages 0-13 are closed; stage 14 (phase-1 foundation
extensions) is the remaining spec stage. Phase 1 closes
with implementation sweep #1 covering stages 1-14, the
generic-C backend, and a DSL -> IR -> C -> captured-stdout
test suite.

Each entry: status box, stage name, brief signal-level summary,
pointer to detail.

- [x] **Stage 0: Architectural foundation.** Q1-Q7 closed: the
  cross-stage architectural decisions that govern the rest of
  the work (crate split, `'tcx` boundary, IR layering, integration
  point, tests/serialization, multi-crate strategy, intrinsic
  recognition). Multi-crate three-tier dispatch infrastructure
  rebuilt and verified end-to-end.
  - Stage status file: `architecture.md` (Q1-Q7 contain the full
    rationale; no separate stage-0-status.md).

- [x] **Stage 1: RegseqModel foundation crate.** ID conventions
  (with `CrateId`), StringInterner, SourceFile/Span (file-local),
  DefPath form (full 19-variant `DefPathData`),
  Instance/InstanceKind/monomorphization story, `impl` lifting
  principle, C-semantics framing, top-level `RegseqModel` with
  invariants, F4/F4-2 determinism rules, custom serde, pretty-
  printer scaffold. Spec is round-5-revised after seven review
  passes; implementation shipped across 7 steps; 229 integration
  tests passing.
  - Stage status file: `stage-1/status.md`.

- [partial] **Stage 2: Serialization format.** Format chosen per
  Q5: textual MIR-inspired pretty-printer (primary) + serde-derived
  JSON (secondary). Schema-level work tracks each subsequent stage
  as variants gain Serialize derives.
  - Stage status file: `architecture.md` Q5 (no separate stage-2
    status; this stage is convention-only with no implementation
    surface of its own).

- [x] **Stage 3: Type system.** Specifies the `Ty` enum the
  bridge fills from rustc's `TyKind` and codegen consumes for C
  type declarations. 15 decisions locked (D1, D2, D3, D4, D5,
  D6, D7, D8, D9, D10, D11, D12, D13, D15, D16) plus the S0
  framing principle. D14 (renderer style) deferred to the new
  stage 13 (type-system pretty-printing) -- the renderer's
  input depends on `Ty` content references that only become
  fully populated at stage 11 (`FnSig`) and stage 12 (`Item` /
  DefPath). Closed via SESSION-A/B/C parallel cluster +
  post-iteration cross-cluster Codex sweep (R1-R10, 2026-04-30,
  52 findings closed across the iteration).
  - Stage status file: `stage-3/status.md`.

- [x] **Stage 4: Constants and literals.** Spec-side
  closed and merged across four locked decisions. D-4-1:
  closed 11-variant `Const` enum (Bool / Char / Int / Uint /
  Float / ZST / Bytes / Aggregate / EnumValue / NullPtr /
  Reloc) plus `Byte` three-state and `InstanceRef` 4-tuple
  cross-build identity. D-4-2: per-variant `ConstIdentityKey`
  byte-form schema + three-state-DFS acyclicity validation +
  value-level lookup injectivity ordering. D-4-3:
  reference-typed const handling (`&str` / `&[T]` literals)
  via the new `SyntheticItemKind::ConstMemoryStatic` family,
  with a content-derived `DefPathHash` and the existing
  variant set composing wide-pointer / thin-pointer references
  via `Aggregate([Reloc, Uint])` / `Reloc` (no new `Const`
  variant needed). D-4-K: deferred `render_const` to stage 14
  (latest of its dependencies). Spec contributions merged
  into top-level `spec.md` (post-merge Codex review pass
  R1-R4 closed at APPROVE on 2026-05-03; 12 findings closed).
  - Stage status file: `stage-4/status.md`.

- [x] **Stage 5: MIR places and projections.** Spec-side
  closed and merged across three clusters. **D-5-1**: locked
  6-variant closed `PlaceElem` enum (Deref / Field / Index /
  ConstantIndex / Subslice / Downcast) + per-variant payload
  shapes (Field carries inline `(FieldIdx, TyId)`; Index
  carries `LocalId` not Operand; Downcast carries
  `VariantIdx` only) + `Box<[PlaceElem]>` projection storage
  + two structural invariants (I-Deref-first from
  PostCleanup; I-Downcast-followed-by-Field from
  `PlaceTy::projection_ty` assertion + H4 cross-check).
  Locked at R10 MINOR-with-NIT-only; residuals deferred to
  top-level merger Codex review per the methodology-deviation
  rationale. **D-5-2**: Place validation phase placement
  (rules at stage 5; walk implementation at stage 11). Locked
  at R6 APPROVE. **D-5-K**: `pretty.rs::render_place`
  deferred to stage 11 (retargeted from stage 12 on the
  D-5-2/D-5-K combined R1 BLOCK finding -- `Item` carries no
  field/variant name tables; names live in §11
  `FieldLayout.name` / `VariantLayout.name`, reachable at
  render time via the head local's `LocalDecl.ty`). Locked
  at R6 APPROVE. Spec contributions merged into top-level
  `spec.md` (post-merge Codex review R1-R8, locked at R8
  MINOR-with-NIT-only on 2026-05-04 per the methodology-
  deviation rationale recorded in `review-log.md`; 15
  findings closed). Total stage-5 Codex cost: 24 rounds
  (10 D-5-1 + 6 D-5-2/D-5-K + 8 post-merge); 50 distinct
  findings closed across the trajectory.
  - Stage status file: `stage-5/status.md`.

- [x] **Stage 6: MIR operands.** Spec-side closed and
  merged across a single combined cluster. **D-6-1**: 3-
  variant closed `Operand` enum (`Copy(Place)`,
  `Move(Place)`, `Constant(ConstId)`) plus per-aspect sub-
  decisions D-6-1.1 (variant cardinality), D-6-1.2
  (`RuntimeChecks` bridge-side lowering rule -- resolve
  via `checks.value(tcx.sess)` against the bridge-host
  Q4 single-session, intern as `Const::Bool(b)`, emit as
  `Operand::Constant(ConstId)`; Charon precedent for
  option-(c) lower-and-eliminate), D-6-1.3 (`Constant`
  payload shape -- `ConstId` only, no
  `Box<ConstOperand>` wrapper, drop `user_ty` and operand-
  level `span`), D-6-1.4 (Copy/Move preservation per
  mrustc's regretted `Use(LValue)` collapse and
  rustc/Charon's preservation). **D-6-2**: no operand-level
  interning; operands inline at use sites. **D-6-3**: no
  type-`Copy` enforcement on `Operand::Copy(p)` (rustc's
  pre-drop-elaboration invariant is gone post-drop-
  elaboration; `optimized_mir` is post-drop-elaboration).
  **D-6-K**: `pretty.rs::render_operand` deferred to
  stage 11 alongside `render_place` (D-5-K precedent;
  `Constant` arm late-binds to stage-14 `render_const`).
  All four D-tags LOCKED at R6 MINOR with explicit
  "Lockability: yes" verdict. Outbound expectations filed
  in `stage-9/expectations.md` "From stage 6"
  (`Spanned<Operand>` per-arg span preservation) and
  `stage-11/expectations.md` "From stage 6"
  (render_operand). Spec contributions merged into top-
  level `spec.md` (new §20 + §18.2 update; post-merge
  Codex review R1-R2 closed at R2 APPROVE on 2026-05-04;
  3 findings closed: 2 MINOR + 1 NIT). Total stage-6
  Codex cost: 8 rounds (6 per-stage + 2 post-merge); 20
  distinct findings closed.
  - Stage status file: `stage-6/status.md`.

- [x] **Stage 7: MIR Rvalues.** Spec-side closed and
  merged across a single combined cluster. **D-7-1**:
  9-variant closed `Rvalue` surface (`Use`, `Repeat`,
  `Ref`, `RawPtr`, `Cast`, `BinaryOp`, `UnaryOp`,
  `Discriminant`, `Aggregate`) drops `CopyForDeref` /
  `ShallowInitBox` (cleanup-pass-discharged), `ThreadLocalRef`
  (DSL-surface: TLS unavailable to Linux/Zephyr drivers),
  `WrapUnsafeBinder` (§15.1 type-rejection cascade + no
  C-shape). Sub-decisions D-7-1.1-1.8 cover per-variant
  payload shapes: stage-3 `Provenance` reuse for `Ref`;
  stage-3 `Mutability` reuse for `RawPtr` with bridge-
  collapse `FakeForPtrMetadata -> Mutability::Not`
  (matching rustc codegen `to_mutbl_lossy()`); 11-variant
  `CastKind` + 4-variant `PointerCoercion` (drops
  `CoercionSource` and `hir::Safety` annotations); flat
  `BinaryOp(BinOp, Operand, Operand)` storage; 7-variant
  `AggregateKind` with three-way Adt split per stage-3
  NominalKind taxonomy + `Tuple(TyId)` for symmetry +
  `Closure(TyId)` referencing identity view per §13.1;
  drops Coroutine / CoroutineClosure (§15.1 type-rejected
  + StateTransform-discharged) and `UserTypeAnnotationIndex`
  slot. **D-7-2**: no Rvalue interning; inline at use sites.
  **D-7-3**: Rvalue-site invariants -- 7-row rejection table
  + 6 positive invariants; walk-enforcement deferred to
  stage 11; F4 / F4-2 cross-build identity OUT of scope.
  **D-7-K**: `render_rvalue` deferred to stage 11 alongside
  `render_place` and `render_operand`; Aggregate field /
  variant names resolve directly from per-variant TyId via
  §12 type interner (no LocalDecl.ty needed); operand-
  rendering subtree is the deferral driver. Spec
  contributions merged into top-level `spec.md` (new §21 +
  §18.2 update; post-merge Codex review R1-R5 closed at R5
  APPROVE on 2026-05-04; 10 findings closed). Total
  stage-7 Codex cost: 10 rounds (5 per-stage + 5
  post-merge); 29 distinct findings closed (5 BLOCK + 24
  MINOR + 0 NIT).
  - Stage status file: `stage-7/status.md`.

- [x] **Stage 8: MIR statements.** Spec-side closed and
  merged across a single combined cluster.
  **D-8-1**: closed 3-variant `StatementKind` surface
  (`Assign(Place, Rvalue)`, `SetDiscriminant(Place,
  VariantIdx)`, `Intrinsic(NonDivergingIntrinsic)`)
  with sub-decisions D-8-1.1-1.4 covering variant
  cardinality and per-variant payload shapes. The 10
  distinct dropped rustc variants reject-on-encounter
  at the bridge with diagnostics naming the
  responsible cleanup-pass / absent build flag (12-row
  table; the `RemoveStorageMarkers` enable-gate
  question raised during kickoff was resolved via the
  bridge driver's `mir_enable_passes` whitelist
  per `crates/regseq-driver/src/callbacks.rs:25-66` +
  the pass-manager override semantics at
  `rustc_mir_transform/src/pass_manager.rs:227-238`).
  **D-8-2**: no `Statement` interning; inline storage
  in stage-10 basic-block storage; F4 / F4-2
  cross-build identity OUT of scope. **D-8-3**:
  wrapper struct `Statement { kind: StatementKind,
  span: Span }`; drop `source_info.scope` (debuginfo-
  only); drop `debuginfos: StmtDebugInfos` entirely
  (no DWARF emission target); the distinct body-level
  `var_debug_info` named-local mapping outbound to
  stage 11. **D-8-4**: 5-part invariants registry
  (Part A 12-row rejection table grouped by discharge
  mechanism; Part B 8 positive invariants -- B-0a
  Place inherited from §19.7, B-0b Rvalue inherited
  from §21.5, B-0c Operand inherited from §20.5,
  B-1 Assign type-equality, B-2 SetDiscriminant
  variant_index in-range, B-3 SetDiscriminant target
  `Ty::Enum`, B-4 Intrinsic::Assume operand bool, B-5
  Intrinsic::CopyNonOverlapping deref-able-pointer +
  pointee-equality + count-as-usize narrowed to
  `Ty::Ptr` / `Ty::Ref` per stage-3 §15.1's `Box<T>`
  rejection cascade; Part C walk-impl deferred to
  stage 11; Part D F4/F4-2 OUT of scope; Part E peer
  comparison informational). **D-8-K**:
  `render_statement` deferred to stage 11 alongside
  `render_place`, `render_operand`, `render_rvalue`;
  three-item outbound expectation bundle to file in
  `stage-11/expectations.md` "From stage 8"
  (render_statement deferral; Statement walk
  implementation; named-local preservation). Spec
  contributions merged into top-level `spec.md` (new
  §22 Statement vocabulary section + §18.2 per-method
  ownership row update for `render_statement`);
  post-merge Codex review R1 closed at MINOR with
  `Lockability: yes` on 2026-05-05 (5 findings
  closed: 0 BLOCK + 4 MINOR + 1 NIT). Total stage-8
  Codex cost: 5 rounds (4 per-stage + 1 post-merge);
  20 distinct findings closed (3 BLOCK + 16 MINOR +
  1 NIT). Notable substantive trajectory: per-stage
  R1 BLOCK caught the `Ty::Adt(NominalId)` umbrella
  fabrication; per-stage R2 BLOCK narrowed B-5 from
  rustc's full `builtin_deref(true)` admissible set
  to RegseqModel-representable subset; post-merge R1
  corrected `SetDiscriminant` codegen claim (writes
  `VariantLayout.tag_value: Option<u128>` per §11.1).
  - Stage status file: `stage-8/status.md`.

- [x] **Stage 9: MIR terminators.** Spec-side closed
  and merged across a single combined cluster. **D-9-1**
  (closed 6-variant `TerminatorKind` -- Goto, SwitchInt,
  Return, Unreachable, Call, Assert; sub-decisions
  D-9-1.1-1.6 covering cardinality + per-variant
  payloads). **D-9-2** Drop-as-Call collapse at the
  bridge (with concrete `flavor=DropGlue` vs dyn-Trait
  `flavor=Virtual + virtual_idx=Some(0)` split mirroring
  rustc `InstanceKind::Virtual(drop_fn.def_id(), 0)`).
  **D-9-3** the load-bearing decision locked at
  candidate (b) `CallCallee::{Direct(InstanceId),
  Indirect(Operand)}` -- closes stage-1 inbound vtable-
  indirect dispatch + stage-3 D5; triggered §9.4 schema
  extension (`virtual_idx: Option<u32>` field on
  `InstanceKind::Function`). **D-9-4** drop per-arg
  spans entirely (rustc_public-aligned at
  `mir.rs:752`); supersedes stage-6 D-6-1.3 inbound.
  **D-9-5** 8-variant AssertKind subset. **D-9-6**
  SwitchTargets explicit-split storage. **D-9-7**
  inline Terminator storage. **D-9-8** wrapper
  `{ kind, span }`. **D-9-9** 9-row rejection table
  (6 pipeline-discharged + 2 DSL-only + 1 collapsed).
  **D-9-10** 4-part invariants registry with 11
  positive invariants (B-0a/c inherited from §19.7 /
  §20.5 + B-1-B-9 introduced; B-9 added at post-merge
  R5 for target/callee-output coherence). **D-9-K**
  render_terminator + 4 sub-renderers
  (render_call_callee, render_switch_targets,
  render_assert_kind, plus an internal
  render_terminator_kind helper) deferred to stage 11
  alongside render_place / render_operand /
  render_rvalue / render_statement. Spec contributions
  merged into top-level `spec.md` (new §23 Terminator
  vocabulary section + §9.4 Function variant +
  `virtual_idx` field + §3.3 step 8 biconditional
  validation + §18.2 per-method ownership rows for the
  4 deferred renderers + §20.3 supersession note);
  post-merge Codex review pass R1-R6 closed at R6
  MINOR-with-NIT-only-and-Lockability:yes on
  2026-05-05. Total stage-9 Codex cost: 12 rounds (6
  per-stage + 6 post-merge); 51 distinct findings
  closed.
  - Stage status file: `stage-9/status.md`.

- [x] **Stage 10: MIR basic blocks.** Spec-side closed
  and merged across a single combined cluster. **D-10-1**
  closed 2-field `BasicBlock { statements:
  Box<[Statement]>, terminator: Terminator }` (cardinality
  + 6 sub-decisions covering field shapes and
  drops). **D-10-2** locked bare `IndexVec<BlockId,
  BasicBlock>` storage with the **principled
  model-vs-analysis line** (model stores facts the
  bridge owns or rustc has resolved; not analyses
  derivable via standard CFG/SSA algorithms; RPO is
  the single carve-out as the storage layout itself).
  **D-10-3** introduced `BlockId` typed-index via
  `newtype_index!` + macro const-body arm extension +
  size/niche assertions + `bbN` sigil convention,
  closing all 4 stage-1 inbound expectations.
  **D-10-4** locked B-RPO storage order + B-Entry
  `START_BLOCK = BlockId(0)` (both inherited from
  `ReorderBasicBlocks` cleanup whitelist + START_BLOCK
  fixed-point at `prettify.rs:33`). **D-10-5** locked
  the 5-part invariants registry (3-row rejection
  table bridge-enforced; 8 positive invariants;
  walk-impl deferred to stage 11; F4/F4-2 OUT; Part E
  forward-looking). **D-10-K** deferred
  `render_basic_block` to stage 11, closing the D-5-K
  through D-9-K renderer-deferral cascade. Spec
  contributions merged into top-level `spec.md`: new
  §24 Basic-block vocabulary section + new §23.1.4
  `Terminator.successors()` per-variant successor
  enumeration helper (added at post-merge R1 as a
  stage-10-driven refinement of stage-9 content) +
  §22 forward references updated `Vec<Statement>` ->
  `Box<[Statement]>` + §23.5 vocabulary preview
  replaced with forward-reference to §24 + §18.2
  per-method ownership row for `render_basic_block`.
  Per-stage Phase B closed at R5 MINOR-with-Lockability:
  yes-and-close-out:YES (5 rounds: R1=7, R2=5, R3=3,
  R4=2, R5=2); post-merge tier-3 review closed at R4
  MINOR-with-close-out:YES (4 rounds: R1=4, R2=2, R3=2,
  R4=2). Total stage-10 Codex cost: 9 rounds (5
  per-stage + 4 post-merge); 29 distinct findings
  closed (4 BLOCK + 16 MINOR + 9 NIT). Five-item
  outbound bundle filed in `stage-11/expectations.md`
  "From stage 10".
  - Stage status file: `stage-10/status.md`.

- [x] **Stage 11: MIR bodies.** Spec-side closed and merged
  2026-05-05 across a single combined cluster of all 10 D-tags
  (D-11-1 Body 4-field shape; D-11-2 LocalDecl 4-field shape
  with `debug_name` lift per stage-8 D-8-3; D-11-3 FnSig
  Item-side reframe (supersedes stage-1 inbound's Body-side
  framing per "Spec is mutable"; 4-peer convergence); D-11-4
  LocalId via `newtype_index!` + `RETURN_PLACE = 0`; D-11-5
  no BodyId, bodies as `Item.body: Option<Body>` structural
  field; D-11-6 joint validation visitor with 10 visit/super
  hook pairs; D-11-6.1 construction invariants 1+9 fold-in;
  D-11-7 hybrid (c) timing -- Drop-as-Call at bridge construction
  time + vtable-indirect rewrite as post-construction pass,
  with cross-stage friction documentation against stage-5
  D-5-1 lacking PtrMetadata PlaceElem; D-11-8 single push API
  on Body + synthetic-local helper; D-11-K categorical
  rendering deferral retargeting D-5-K through D-10-K +
  stage-1 _N sigil + stage-10 bbN sigil to stage 13). Spec
  contributions merged into top-level `spec.md` (new §25
  Body-level vocabulary section; §14 FnSig placement note;
  §18.2 per-method ownership table reframed for D-11-K
  retargeting; §19.8 / §20.6 / §21.8 / §22.8 / §23.9 / §24.9
  per-stage renderer deferral notes flipped to stage 13;
  §3 RegseqModel + §3.x validation + §8 ItemKind + §9.4
  InstanceKind + §18.1 sigil table + §23.6.2 callee_fn_sig
  helper + §23.2.3 Return + §24.2.1 IndexVec convention all
  updated for the structural reframes); post-merge tier-3
  Codex review R1-R5 closed at R5 APPROVE on 2026-05-05.
  D-5-K, D-6-K, D-7-K, D-8-K, D-9-K, D-10-K + D-10-K.1/.2/.3
  flipped to `[REOPENED]` in `decisions.md` with annotation
  `(retargeted to stage 13 per D-11-K)`. Total stage-11
  Codex cost: 11 rounds (6 per-stage + 5 post-merge); 26
  distinct findings closed (9 BLOCK + 17 MINOR + 0 NIT).
  Outbound bundles filed: `stage-12/expectations.md`
  "From stage 11" (Item.kind=Function schema expansion +
  invariant 9 closure); `stage-13/expectations.md`
  "From stage 11 (per D-11-K)" (12-item rendering bundle
  with predecessor-stage attribution).
  - Stage status file: `stage-11/status.md`.

- [x] **Stage 12: Rust Items.** Spec-side closed and merged
  2026-05-06 across a single combined cluster of all 7
  D-tags (D-12-1 Item schema + ItemKind shape with **Option B
  reframe** of stage-11 D-11-3 + D-11-5 [moves fn_sig + body
  from Item to InstanceKind::Function, restoring symmetry];
  D-12-1.2 (c) mutability moves from InstanceKind::Static to
  ItemKind::Static per stage-1 intent; D-12-1.3 Item-level
  typed fields per stage-1 R5.2 with closed 4-variant Linkage
  + InlineHint enums; D-12-1.5 DomainAttr forward-pointer to
  stage-14 sibling tables; D-12-1.6 hybrid as_function()
  accessor + named FunctionInstanceData struct; D-12-1.8
  13-helper Instance-side pull-through method inventory;
  D-12-2 foreign-item dispatch rules; D-12-3 lang-item
  recognition mechanism with closed 3-flag set + AdtFlags
  predicates; D-12-4 construction-invariant closure for
  stage-1 deferrals 3 / 4 / 6 with BridgeInput parameter on
  validate; D-12-5 canonical 5-phase bridge construction-
  pipeline order with new §3.4 subsection; D-12-6
  render_def_path stub fill; D-12-K categorical Item-level
  rendering deferral to stage 13 compositional with D-11-K
  cascade). Spec contributions merged into top-level
  `spec.md` (in-place edits to §3 + §3.1 + §3.3 + §8 + §9.4
  + §13.1 + §13.3 + §14.3 + §18.2 + §23.6.2 + §25.4 + §25.5
  + §25.7 + new §3.4 + §3.5; stage-11 D-11-3 + D-11-5
  flipped to [REOPENED] in `decisions.md` with annotation
  `(revised in stage 12 per D-12-1.4)`); post-merge tier-3
  Codex review pass R1-R3 closed at R3 APPROVE on
  2026-05-06. Total stage-12 Codex cost: 10 rounds
  (7 per-stage + 3 post-merge); 31 distinct findings closed
  (20 BLOCK + 11 MINOR + 0 NIT). Outbound bundles filed:
  `stage-13/expectations.md` "From stage 12 (per D-12-K)"
  (7-entry Item-level rendering bundle); cross-stage
  finding F-12-Reify (rustc ReifyShim payload drift; flagged
  for next stage-9 review).
  - Stage status file: `stage-12/status.md`.

- [x] **Stage 13: Pretty-printing.** Spec-side closed and
  merged 2026-05-07 across all 8 main D-tags + 41
  sub-decisions + 1 closed-by-absorption (D-13-8 → D-13-9.6).
  D-13-1 (master rendering style + method shape:
  Rust-source-style with sigil decorations + `PrettyPrinter`
  methods returning `String`; D14 closes by absorption);
  D-13-2 (body envelope + indentation + sigil placement);
  D-13-3 (span-rendering mode + debuginfo gating via
  `PrettyPrintOptions` toggle); D-13-4 (Item-level
  rendering shape + building-block-vs-composer
  architectural division); D-13-5 (`render_ty` per-arm
  specs across the closed 19-variant `Ty` enum); D-13-6
  (body-level renderer per-arm specs across 9 public + 1
  internal); D-13-7 (worked-example specimens for snapshot
  stability; ~75 specimens at per-arm spec level with
  policy-bucket exception); D-13-8 [CLOSED-BY-ABSORPTION]
  (sigil + ownership table consolidation absorbed into
  D-13-9); **D-13-9 the load-bearing emergent decision**
  (pretty-printer spec moves to top-level companion file
  `pretty-printer.md`; `spec.md` §18 becomes
  one-paragraph pointer; methodology rule extension to
  README §5.9 rule 6 + §5.5 step 5/6/7 + §4 folder
  layout). Spec contributions merged into top-level
  `spec.md` (D-13-9.2 cross-reference edits at §10.4 /
  §18 / §19.8 / §20.6 / §21.8 / §22.8 / §23.9 / §24.9;
  ~375 lines pruned) AND new top-level `pretty-printer.md`
  (created fresh; ~1532 lines per D-13-9.4 10-section
  structure); post-merge tier-3 Codex review pass R1-R4
  closed at R4 APPROVE / 0 issues / Lockability:yes on
  2026-05-07 (8 distinct findings closed across the
  4-round trajectory: 6 → 1 → 1 → 0). Stage-3 D14 flipped
  to `[CLOSED-BY-ABSORPTION]` in `decisions.md` per
  D-13-1.3 with annotation `(absorbed into D-13-1 per
  stage 13)`. Total stage-13 Codex cost: 8 rounds (4
  per-stage Phase B + 4 post-merge tier-3); 39 distinct
  findings closed. Stage-13 lands tighter than stage-9
  (12/51), stage-12 (10/31), and stage-11 (11/26).
  - Stage status file: `stage-13/status.md`.

- [in progress] **Stage 14: Phase-1 foundation extensions.**
  Opened 2026-05-12. **D-14-1 + D-14-3 LOCKED at per-stage
  Phase A (2026-05-12; 3 Codex rounds; 8 findings closed).**
  Remaining clusters open: D-14-2 (diagnostic-logging
  vocabulary; NEXT), D-14-4/5/6 (composer renderers;
  pretty-printer-snapshot.md edits start here), D-14-7/8/9
  (external-review Findings #2/3/4 surgical fixes),
  `core::ffi::c_void` rejection enforcement, post-cluster
  cross-validation Codex review. Detailed state +
  next-action in `stage-14/status.md`. Original cluster
  inventory (informational; D-14-1 + D-14-3 contents now
  superseded by what landed in `stage-14/discussions.md`):
  (a) Q7 intrinsic-recognition mechanism correction per
  external-review Finding #1 -- tool-namespace attribute
  walk rooted by `#![register_tool(rseq)]`, NOT
  `tcx.intrinsic_raw(did)`; closed `IntrinsicKind`
  vocabulary; unknown names rejected.
  (b) Resolved-intrinsic storage on `RegseqModel`:
  `intrinsic_items: IndexVec<ItemId, Option<IntrinsicInfo>>`
  (or equivalent). First concrete instance of the
  DomainAttr sibling-table architecture.
  (c) Initial `IntrinsicKind` entries: diagnostic-logging
  family only (e.g., `log_debug`, `log_info`, `log_warn`,
  `log_error` -- exact names + arity TBD via Codex review).
  MMIO / atomic / interrupt / clock entries deferred to
  their respective phase-2 stages.
  (d) DomainAttr sibling-table architecture on
  `RegseqModel`: key-kind selection rule (`ItemId` /
  `InstanceId` / `FieldPath` / `Span`); the `IndexVec`
  storage pattern; bridge population + validation rules.
  Spec is abstract; only the intrinsic-table is
  instantiated in stage 14.
  (e) 3 composer renderers per stage-13 D-13-4.6:
  `render_const(ConstId) -> String`,
  `render_instance(InstanceId) -> String`, top-level
  `render(&self) -> String`. Land in top-level
  `pretty-printer.md` (co-equal merger artifact per D-13-9).
  (f) External-review Finding #2: reject `#[naked]` in
  v1; remove `is_naked` Item field. Reopens D-12-1.3 +
  D-9-9 + D-13-4.3.
  (g) External-review Finding #3: retarget the stale
  "stage 13 (codegen)" forward pointer in `spec.md:1892`
  to C-backend territory.
  (h) External-review Finding #4: Specimen 70 split in
  `pretty-printer.md`.
  - Stage status file: `stage-14/status.md` (to be created
    during stage open per README §5.1 step 7).

- [ ] **Phase-1 implementation sweep.** Implement stages
  1-14 against the locked spec. Single sweep covering full
  Ty closure, MIR-derived RegseqModel surface, bridge from
  rustc's `optimized_mir`, pretty-printer, Q7 intrinsic
  mechanism, DomainAttr sibling tables. Stage 1's existing
  impl is grandfathered (per README §2 "Spec before
  implementation"); stages 3-14 implement fresh.

- [ ] **Phase-1 generic-C backend.** Hosted-userspace
  lowering: libc, `main()`, `printf`, host-platform integer
  types. Diagnostic-logging primitives lower to
  printf-family. Separate codegen path from the Linux kernel
  backend per README §2 "Dual backend" rule.

- [ ] **Phase-1 test suite.** DSL -> IR -> C ->
  captured-stdout integration tests; insta snapshots of IR +
  C output; foundation behavioural assertions via
  diagnostic logging. Test infrastructure for compile-then-
  run host-binary validation.

## Phase 2 -- iterative domain extensions

Lazy stage allocation; each stage covers one coherent
domain family. Closes spec-side under standard merger
contract (README §5.5) plus an incremental implementation
pass extending the compiler + Linux kernel backend + test
suite. No fixed stage count; queue grows as the next
domain family becomes load-bearing.

- [ ] **Stage 15: regseq-trait scaffolding.** Foundation
  for regseq-provided glue traits (`Lifecycle` as the
  first concrete instance) + driver-struct discovery
  mechanism + DT-binding metadata schema + `RegseqError`
  taxonomic enum. Enables driver entry/exit + status
  taxonomy + device binding without any device I/O.
  Inbound forward refs land in
  `stage-15/expectations.md` (created lazily at stage open).

- [ ] **Stage 16: MMIO family.** `mmio_*` intrinsic
  vocabulary entries + `Mmio<T>` resource type +
  `#[rseq::mmio(...)]` declarative attribute family +
  volatility codegen rule + Linux-backend MMIO lowering
  (`readl` / `writel` / `__iomem` qualification).
  Closes inbound expectations from stage-3 D9 +
  external-review Finding #1 (MMIO portion). Inbound
  forward refs land in `stage-16/expectations.md`.

- [ ] **Stage 17: interrupt / ISR family.**
  `#[rseq::interrupt(...)]` attribute family + ISR
  identification + Linux-backend ISR scaffolding
  (`request_irq`, threaded handlers).

- [ ] **Stage 18: atomic family.** `atomic_*` intrinsic
  vocabulary entries + ordering encoding choice per
  stage-3 D10 inbound + Linux-backend atomic codegen
  mapping (`atomic_*` suffix family + `READ_ONCE` /
  `WRITE_ONCE` / `smp_load_acquire` / `smp_store_release`).
  Closes inbound expectations from stage-3 D10.

- [ ] **Stage 19+: clock / reset, register-bank, ...**
  TBD by device-pressure findings.

## Phase 3 -- device pressure

Tiered device fixture corpus; runs in parallel with phase 2
from stage-15 close onward. Each milestone exercises the
phase-2 stages that have landed by that point. Findings
escalate back to phase-2 / phase-1 spec when needed.

- [ ] **Hello World QEMU toy device + minimal Linux
  DT-binding driver.** Exercises stage-15 surface
  end-to-end: Rust DSL -> bridge -> RegseqModel ->
  Linux-backend C -> kernel module -> QEMU boot ->
  device-tree bind -> driver-load -> log emission.

- [ ] **Simple MMIO-only QEMU device.** Exercises
  stage-15 + stage-16 surface.

- [ ] **pl011 in QEMU.** Exercises stage-15 + stage-16 +
  stage-17 + stage-18 + stage-19 surface as those land.
  First-body candidates from the pl011 fixture:
  `field_val` (~36 lines), `raw_reg_val` (~22 lines),
  `mask_all_interrupts` (~22 lines, register access).
  Fixture + tooling already in place under
  `tests/fixtures/pl011-full-spec/`.

- [ ] **Beyond pl011: additional driver fixtures** as
  project scope expands. See `../manual-lowering/README.md`
  for phase-3 specifics.

## Active focus

**Strategy revision (2026-05-12).** The original monolithic
"Stage 14: Domain-specific extensions" plan is superseded
by a 3-phase methodology with per-domain-family stage
allocation in phase 2 (see README §1.3 + §2's "Domain
extensions iterate per stage in phase 2" and "Dual backend"
rules). Stage 14 narrows to **phase-1 foundation extensions**
(Q7 mechanism + diagnostic-logging vocabulary + DomainAttr
sibling-table architecture + 3 composer renderers +
external-review Findings #1-#4). Phase 1 then closes with an
implementation sweep, the generic-C backend, and a DSL
test suite. Phase 2 proceeds incrementally -- one stage per
domain family -- with phase 3 device pressure running in
parallel from stage-15 close onward. Rationale: domain
extensions are tightly coupled (resources + intrinsics +
attributes + abstraction types per family) and an
open-ended surface; spec-only enumeration upfront is liable
to bake in shapes that break under real device pressure.
Per-family stages with implementation + phase-3 device
pressure feedback after each is the safer cadence.

**Stage 14 is the open spec stage.** All earlier spec stages
(0-13) are closed; their per-stage status files
(`stage-N/status.md`) and frozen `stage-N/spec-snapshot.md`
artifacts remain as historical records. The canonical
RegseqModel spec lives at top-level `spec.md`; the canonical
pretty-printer spec lives at top-level `pretty-printer.md`
(co-equal merger artifact per stage-13 D-13-9).

**Stage 14 cluster #1 (D-14-1 + D-14-3) LOCKED 2026-05-12
at per-stage Phase A.** Closed across 3 Codex rounds; 8
findings closed (1 BLOCK + 4 MINOR + 3 NIT). Significantly
tighter than every prior stage's Phase B (stage-13: 4/39;
stage-12: 7/31; stage-11: 6/26; stage-10: 5/29). Detail
in `stage-14/discussions.md` (audit shape) +
`stage-14/review-log.md` (R1-R3 trajectory). What landed
structurally: the `DomainAttrTables` architecture +
`AttrData` discriminated union + `IntrinsicKind`
closed-vocabulary enum + Q7 recognition codepath +
14th Instance-side pull-through helper `instance_intrinsic`
+ §8.0 forward-pointer correction per Finding #1 +
two-phase population protocol (Phase 2 recognition into
`HashMap<DefPathHash, IntrinsicKind>` working cache +
Phase 4 materialization into final sibling tables;
mirrors §3.4 `vtables` / `ConstMemoryStatic` pattern).
Active focus advances to D-14-2 (diagnostic-logging
vocabulary). No `[REOPENED]` flips needed on prior stages;
stage-4 §1287-1299's 4-key-kind forward-pointer was
refined to 3 tables in-place per §5.9 rule 1, not
reopened.

**Implementation discipline (revised).** Within any single
stage, spec and implementation do not interleave (the
intra-stage protection from README §2 stands). Between
stages, implementation passes happen against the just-locked
spec: **implementation sweep #1 follows stage-14 close**
(covering stages 1-14 in one sweep, producing the
domain-agnostic compiler + the generic-C backend + the
phase-1 test suite); **subsequent phase-2 stages each get
an incremental implementation pass on close**, extending
the compiler + the Linux kernel backend (separate from the
generic-C backend per the "Dual backend" rule) + the test
suite. Stage 1's existing impl is grandfathered. Phase-3
device-pressure work runs in parallel with phase 2 from
stage-15 close onward.

**Phase boundaries.** Phase 1 closes when stage 14 has
locked, the implementation sweep has shipped, the generic-C
backend produces working hosted-userspace programs, and the
DSL -> IR -> C -> captured-stdout test suite is green.
Phase 2 has no closure event -- it is the iterative
phase that proceeds family-by-family. Phase 3 has no
closure event either -- it is the iterative device-pressure
phase that grows with the fixture corpus. The overall
project's "v1 done" criterion is **panic-set empty against
the v1 device fixture corpus** (initially Hello World QEMU;
expanding through pl011 and beyond).

## Open architectural decisions

None at present. Q1-Q7 are closed in `architecture.md`. Future
decisions land there as Q8, Q9, ... with references to evidence
and rationale.

## Blockers / waiting on

(none)

## Deferred items (cross-stage)

Lightweight follow-ups that span multiple stages or have no clear
later-stage owner. Stage-specific deferrals live in the relevant
`stage-N/expectations.md` (forward refs) or
`stage-N/status.md` (per-stage open items).

- **`StringInterner` deserialize-path optimization.** Step 4 of
  stage-1 added the `serde` `rc` feature to derive serde for
  `Option<Arc<str>>` on `SourceFile.content`. The `StringInterner`
  deserialize path still goes through `Vec<String>` (one extra
  allocation per entry); could be refactored to `Vec<Arc<str>>`
  directly using the `rc` feature. Codex flagged as
  optional/non-blocking.

- **Construction invariants 3, 6 (and `spec_crate_stable_id`
  half of invariant 4): CLOSED at stage 12.** Per D-12-4.1
  (F4 sort monotonicity walk), D-12-4.2 (`def_path.crate_id`
  bound check), and D-12-4.3 (`BridgeInput { spec_crate_stable_id }`
  parameter on validate). Phase B step 3 reordered to (a)..(g)
  to incorporate the three checks. Construction invariant 9
  remains stage-11-owned (Direct(InstanceId) closure check
  in joint-validation visitor); stage 12 owns
  `model.instances` integrity (one-to-one with Items;
  populated by collector; exhausted by validator). The
  in-source comments naming "stage 12" can be cleaned up at
  end-of-phase-1 implementation sweep.

- **`newtype_index!` macro const-body arm.** Currently the macro
  accepts only empty-body invocations. Closed by stage-10
  D-10-3.2 (extending the macro grammar) + stage-11 D-11-4.2
  (consuming the arm for `LocalId::RETURN_PLACE = 0`).

## Next action

**Continue Stage 14: open D-14-2 cluster
(diagnostic-logging intrinsic vocabulary).** Per
`stage-14/status.md` "Next action" — the per-stage
tracker owns the detailed scope and entry steps. D-14-2
is compositional with D-14-1.5's already-locked
`IntrinsicKind::Log(LogLevel)` shape; scope covers:
- Vocabulary entries finalization (confirm or refine the
  working `LogLevel` variant set + NAME mapping
  currently provisional in `stage-14/spec-snapshot.md`
  §3.6.3).
- Argument shape (just `&str`? + value-args? format-string
  carrier?). Load-bearing for what payload the intrinsic
  call site carries in the IR and what the backend has to
  render.
- Macro vs direct call (pins the post-expansion intrinsic
  shape that the bridge sees; the macro itself is
  facade/impl-sweep territory).
- Rust-side stub function signatures in the regseq
  facade.
- Backend lowering contract (informational; codegen is
  impl-sweep territory).

Open `stage-14/status.md` for the detailed D-14-2 scope
+ operational steps; `stage-14/memory.md` for the
stage-wide D-tag discussion agenda; `stage-14/discussions.md`
for D-14-1 + D-14-3 audit; `stage-14/review-log.md` for
the R1-R3 Codex trajectory; `stage-14/prior-art/rustc.md`
for the Q1-Q4 rustc evidence file.

**Stage 14 cluster queue (revised at D-14-1 + D-14-3 lock):**

1. ~~D-14-1 + D-14-3~~ — **LOCKED 2026-05-12** (per-stage Phase A; 3 Codex rounds; 8 findings closed).
2. **D-14-2** — diagnostic-logging vocabulary. **NEXT.** Small surface; ~2-4 Codex rounds expected.
3. D-14-4 / D-14-5 / D-14-6 — composer renderers (sequential dependency: `render_const` → `render_instance` → top-level `render`); pretty-printer-snapshot.md edits start here.
4. D-14-7 / D-14-8 / D-14-9 — external-review Findings #2/3/4 surgical fixes; small; can land alongside composer clusters.
5. `core::ffi::c_void` rejection enforcement — spec edits only; no D-tag.
6. Per-stage post-cluster cross-validation Codex review on the full `stage-14/spec-snapshot.md` + `stage-14/pretty-printer-snapshot.md`.
7. Merger contract (README §5.5) — integrate stage-14 contributions into top-level `spec.md` + `pretty-printer.md`; flip prior-stage D-tags per Finding #2 reopens; post-merge tier-3 Codex review; freeze stage-14 snapshots.

**Post-stage-14 queue** (phase-1 close + phase-2 open):

1. **Implementation sweep #1.** Implement stages 1-14
   against the locked spec; Stage 1's existing impl is
   grandfathered; stages 3-14 implement fresh.
2. **Generic-C backend** (hosted userspace, libc,
   `main()`, `printf`). Diagnostic-logging primitives
   lower to printf-family. Separate codegen path per
   README §2 "Dual backend" rule.
3. **Phase-1 test suite.** DSL -> IR -> C ->
   captured-stdout integration tests; insta snapshots
   of IR + C output.
4. **Open Stage 15: regseq-trait scaffolding.** First
   phase-2 stage. Spec + incremental implementation
   pass (compiler + Linux kernel backend + test suite
   extension).
5. **Open phase-3 milestone: Hello World QEMU toy
   device.** Runs in parallel with stage 15's
   implementation. End-to-end driver validation.
6. **Open Stage 16: MMIO family.** Spec + incremental
   implementation pass.
7. **Open phase-3 milestone: Simple MMIO-only QEMU
   device.** Runs in parallel with stage 16's
   implementation.
8. **Continue iteratively** -- stages 17 / 18 / 19+
   interleave with phase-3 fixture milestones; pl011
   becomes a phase-3 milestone after MMIO + interrupt
   + clock have landed.

**Project v1 completion criterion.** Panic-set empty
against the v1 device fixture corpus (initially Hello
World QEMU; expanding through pl011 and beyond). No
fixed stage count; phase 2 + phase 3 iterate until
the criterion holds.

## How to update this file

- Mark stages `[x]` (closed), `[ ]` (not started), `[partial]`
  (started with brief note), or `[in progress]` (active focus).
- Update `Last updated` at the top when changes land.
- Stage-specific detail goes in `stage-N/status.md`,
  not here.
- Deferred items here are cross-stage only; stage-specific
  deferrals live in the relevant stage's status file.
- Stage status files are created lazily: a stage gets one when
  it becomes active, when it closes (preserving history), or
  when it accumulates detail worth tracking. Stages 0 and 2 do
  not have separate status files because their detail lives
  fully in `architecture.md`.
