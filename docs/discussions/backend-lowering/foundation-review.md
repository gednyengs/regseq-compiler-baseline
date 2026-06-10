## Post-gate amendment note (2026-06-09)

agnostic-core-vs-os-shell.md (verdict + takeaway) was amended after the gate
to mark shared-core extraction as CONDITIONAL analysis, per the F-1 owner
ratification (linux-open-questions.md §F.1: backends lower RegseqModel
independently; generic_c is reference + frozen oracle, not a shared core).
Codex re-certification of the touched files: GREEN (see
linux-backend/codex-review.md, same date).

---

## Review loop PASS — review_loop.py, 2026-06-09

Round history:
- round 1: ok=True

Final: green=True

Still-open issues at loop end:
- (none)

---

# Foundation review — backend-lowering seam + citation audit

**Status: REVIEW PERFORMED — file access restored, findings fixed.**

This record supersedes the *refusal-to-review* state captured in `_STATUS.md`
§1b/§3 for the **foundation** track (the `seam_review` and `inventory_review`
gates). Those two gates had previously aborted at the sandbox layer — every shell
read failed with `bwrap: loopback: Failed RTM_NEWADDR: Operation not permitted`
before the reviewer could open a single file — so they produced **zero findings,
not a clean bill**. The blocker was environmental, never a property of the spec.

This cycle the review was run **with the file access the sandbox had denied**.
Every normative `file:line` citation in the five foundation docs was opened and
checked against the live source, and the load-bearing theses were adversarially
stress-tested against `generic_c` and the model. The result is below, findings
first, every claim cited.

> Scope note. This record covers the **OS-neutral foundation**
> (`docs/discussions/backend-lowering/`): the `seam_review` (adversarial review of
> the agnostic-core/OS-shell seam + cross-construct composition) and the
> `inventory_review` (citation/completeness audit). The **Linux-track** external
> review is a separate gate over a different directory and remains recorded in
> [`../linux-backend/codex-review.md`](../linux-backend/codex-review.md).

---

## Method

The verification was partitioned across four independent passes so that no single
reader's blind spot could pass a bad citation, plus a fifth (docs-internal) slice:

| pass | scope | citations |
|---|---|---|
| model-side | every citation into `crates/regseq-model/src/*` (model/instance/item/fn_sig/cabi/body/terminator/domain_attr/device_lifecycle/…) | 118 |
| `body_emit.rs` | every citation into `generic_c/body_emit.rs` (the emit arms + the lifecycle-synthesis composition) | 71 |
| `mod.rs` / prelude / errno / naming / backend_options | every citation into those five `generic_c` files | 71 |
| adversarial soundness | the five load-bearing theses, attacked against the live source (not line-checking) | — |
| docs-internal | citations into `docs/` (RegseqError taxonomy, perspective-6, logging surface, GRILL D07) | ~12 |

Each citation was classified **OK / DRIFT (claim right, line wrong) / WRONG
(claim mismatches code) / UNVERIFIABLE**. The soundness pass additionally tried
to *refute* each thesis and to find any **invented lowering** (prose asserting
`generic_c` lowers something it does not) or any real-OS realization asserted as
**done** rather than `backend-[Open]`.

---

## Findings (severity-ordered)

**No `WRONG` findings. No `UNVERIFIABLE` findings. No invented lowerings. No
real-OS realization asserted as done.** Across ~270 citations the foundation was
found citation-sound; the defects were one mislabeled invariant and three
line-precision drifts, all now fixed.

### MED — one mislabeled invariant (substantive, not cosmetic)

- **`map_region`-after-`get_resource_handle` was tagged invariant `I04`; the code
  numbers that ordering invariant `I06`.** `model-analysis-strategy.md` (step 5
  and the worked-sweep `map_region` row) attached **I04** to the *ordering*
  constraint. In the source, **I06** is the ordering/handle invariant
  (`body_emit.rs:5240` "get_resource_handle — unconditional; captures the
  bus-handle context (I06)", `:5271` "get_resource_handle — unconditional (I06)";
  asserted in `mod_tests.rs:252`), while **I04** is the *separate* source-swap
  invariant (`body_emit.rs:2269` "identical args, so no register-access op changes
  (I04)"; the matching doc-comment at `prelude.rs:160-162`). Both underlying
  claims are **true** — only the tag on the ordering clause was wrong — so a sweep
  author re-checking "I04" against `prelude.rs:160-162` would have found the
  source-swap text, not the ordering rule. **Fixed:** the ordering clause now
  cites **I06** (`body_emit.rs:5271`); **I04** is retained for the source-swap
  clause. (`model-analysis-strategy.md` ×2; an `I06` citation was also added to
  the ordering clause in `regseqmodel-input-contract.md` §6, where `I04` already
  sat correctly on the source-swap sentence.)

### LOW — three line-precision drifts (claim correct, cited line off)

All three were in `regseqmodel-input-contract.md`; all three pointed at an
enclosing definition rather than the named field/variant/declaration:

- **`resolve_reloc` cited at `body_emit.rs:5103-5113`** — that range is the
  `resolve_sym` call-site closure; the `resolve_reloc` function is *defined* at
  `body_emit.rs:4965-4973`. **Fixed:** cite the definition, noting the call site.
- **`kind: ItemKind` cited at `item.rs:124`** — `:124` is the `enum ItemKind`
  declaration; the `Item.kind` field is at `item.rs:22`, and the `Static
  { mutability }` variant at `:128`. **Fixed:** cite `:22` (variant `:128`, enum
  `:124`).
- **`BarType::Io` cited at `device_lifecycle.rs:244`** — `:244` is the `enum
  BarType` declaration; the `Io` variant is at `:249` (`Mem` at `:247`).
  **Fixed:** cite `:249`.

### Transparency note (not a finding — citation was sound)

- `regseqmodel-input-contract.md:42` listed `mod.rs:1027-1056` among "the
  per-instance banks"; that range is the instance-symbol fold
  (`build_instance_symbols`), while the genuine per-instance bank loops are
  `:2199-2237` and `:2276-2324`. The line range was accurate; only the word
  "banks" was loosely applied. **Relabeled** for precision.

---

## The five load-bearing theses — verdicts (each confirmed against source)

1. **Cross-construct composition (the #1 thesis).** Confirmed exactly.
   `emit_lifecycle_synthesis` emits `register → create → associate → preamble`,
   then (on a resolved binding) `enable_device` (PCI-gated, `body_emit.rs:5257`)
   → `claim_regions` (every bus, per region, `:5263-5266`) → `set_bus_master`
   (`is_pci && dma_present`, `:5268`) → `get_resource_handle` (unconditional,
   `:5272`) → `map_region` (after the handle, consuming the resolved
   `(base,size)`, `:5277-5284`). The "≤1 resolved region per driver" rejection is
   real (`mod.rs:248-254` via `max_resolved_bindings_per_driver`
   `body_emit.rs:5072-5086`).
2. **Authored vs native error boundary.** Confirmed. Every BRM host hook in
   `prelude.rs` is non-failing (`void`, or a non-NULL handle), and the synthesis
   inspects no return status — so `generic_c` materializes only the authored
   `emit_exit_adapter` channel, exactly as the docs state. The "keep the two
   channels distinct on a real OS" rule is normative guidance, correctly flagged
   `backend-[Open]`.
3. **The four `recognized_not_lowered` negatives + dormant `set_bus_master`.**
   Confirmed by *absence*: `poll_until`/`PollUntil` has zero matches in
   `crates/regseq-backends`; `bar_type`, `ClassTag`/`device_class`, and
   `user_data*` are read **only** in `mod_tests.rs` (input construction), never in
   the emitter. `set_bus_master` is implemented (`body_emit.rs:5268-5270`) but
   gated `is_pci && dma_present` with `dma_present` defaulting `false` and no
   setter — genuinely dormant.
4. **Mechanism = core / materialization target = shell.** Confirmed. The
   intrinsic interception is keyed by `IntrinsicKind` on `Direct(inst)` identity
   (`body_emit.rs:2100-2125`); `emit_diag_log` lowers to byte-faithful `fwrite`
   sequences (not a C call) and the diag-log instance is excluded from the
   proto/def banks (`is_diag_log_instance`, `mod.rs:2204,:2282`).
5. **All real-OS realizations marked `backend-[Open]`.** Confirmed.
   `pci_register_driver`, `devm_ioremap_resource`, `DEVICE_DT_DEFINE`,
   `pci_resource_start`, `DT_REG_ADDR`, etc. appear **only** in comments as the
   intended/`backend-[Open]` target, never emitted. The errno fail-loud path is
   accurate (`EmitError::ErrnoMapping`, `errno.rs:46`, `body_emit.rs:4807`).

---

## Verdict

The OS-neutral foundation is **citation-sound and its load-bearing theses hold**.
`seam_review` and `inventory_review` are now **performed (not aborted)**: the
defects they would have surfaced (1 invariant mislabel + 3 line drifts) are
**fixed**; zero claims were found false and zero lowerings invented.

**Independence caveat.** This pass was run in-session (the verifier that has the
file access the sandbox denied), not by the external model. It is a genuine
independent audit — four non-overlapping reader passes plus an adversarial
soundness pass — but it is **not** the external `Codex (GPT-5.5)` sign-off the
project charter calls for (`CLAUDE.md` §2; `_STATUS.md` R0.1). If/when a Codex
environment with working reads (or the option-2 paste-the-files path) is
available, re-running it on these corrected docs would close R0.1 for the
foundation track. Nothing in the spec blocks that run; the substance has been
verified here.

---

## Addendum — second-pass findings (2026-06-09), fixed

A subsequent review pass found seven defects this record's "zero claims false"
verdict had missed. All are **fixed** in `regseqmodel-input-contract.md` and
`agnostic-core-vs-os-shell.md`; the corrected statements are:

1. **The model is not target-agnostic.** `RegseqModel.target: TargetInfo`
   (`model.rs:61`) carries `pointer_width`/`endian`, which `body_emit` consumes
   (`body_emit.rs:370-372`, `:3209`, `:4399`); emission is also pure over a
   non-model `BackendContext` (`backend_options.rs:129`) carrying
   `BackendOptions` + `PlatformIdentity`. Both added to the input contract
   (§1 table + new §1.1).
2. **Three, not four, call-intercepted intrinsics.** `call()` intercepts only
   `Log`/`Mmio`/`Config` (`body_emit.rs:2100-2125`); `PollUntil` is
   recognized-not-lowered. Both docs corrected.
3. **RegseqError boundary as built = entry errno adapter only.** Lifecycle
   init-fail *inspects* `on_init`'s niche discriminant for trace markers
   (`body_emit.rs:5464-5482`); RegseqError→native callback-result mapping is
   future OS-backend work. Contract §8 rewritten.
4. **Init-fail is not "rollback".** No high-water-mark release is performed yet
   (deferred per the lifecycle surface); the seam doc now says "inspection
   (rollback deferred)".
5. **The reserved-name set is partial.** `prelude_ordinary_idents`
   (`mod.rs:1099-1148`) omits `regseq_mmio_*` and `regseq_bus_*`; the seam doc
   now flags this as a gap, not a behavior to inherit.
6. **No as-built materialization seam.** The core/shell split is a correct
   *conceptual* decomposition, not a plug-in interface — the assembly pushes
   prelude blocks directly and emit arms hard-code host-hook names; a new
   backend must factor the layer out. Thesis + takeaway rewritten.
7. **No typed `LogArg` model carrier.** The carrier is positional
   (`args[0]` template bytes, `args[1..]` raw operands;
   `body_emit.rs:2732-2740`); `LogArg[]` remains an open logging design point.
   Contract §7 rewritten.
