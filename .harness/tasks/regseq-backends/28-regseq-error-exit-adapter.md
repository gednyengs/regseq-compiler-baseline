# regseq-backends-28: Generic-C RegseqError exit adapter + symbolic switch + int shim (Phase-2a)

> Source: `.claude/specs/regseq-error/PLAN-2026-06-02-000057.md` (tasks **T06+T08** +
> the adapter-switch half of **T10**), contracts **C02/C03**, requirements
> **R01/R03/R04/R07**, invariants **I02/I03/I05**, assumption **A3** / open item
> **O3/O4**. (Resolves Codex round-1 blocking #2.) Golden-defining.

## Description

Emit the boundary adapter that maps an authored `RegseqError` to a libc errno at
the `regseq_main → main` crossing. When the classifier (regseq-backends-27) yields
`ErrResult`, `emit_generic_c` **hand-emits** an in-TU adapter
`<entry>__regseq_exit(void)` that reads the `Result`/`RegseqError` discriminants
per §13 and returns the **symbolic** errno (C02), and `generate_main_c` emits
`int main(void){ return <entry>__regseq_exit(); }`. Includes the **O3
discriminant-read mechanism spike** as its design step.

**Why hand-emit (blocking #2):** model constants are numeric/bytes/relocs (no
symbolic-macro const), and lowered body returns emit `_0`/`return;` — so a
synthesized model-level adapter body **cannot** emit `return EINVAL;` (would
violate I03/C03). The adapter is therefore backend-authored C that **reuses §13
discriminant extraction** for the reads only.

## Input Context

### Code anchors (confirm at HEAD)

- `crates/regseq-backends/src/generic_c/body_emit.rs` — the §13 discriminant
  readers: `discriminant_expr` (~:856), `direct_discriminant_ternary` (~:902),
  `assign_niche_discriminant` (~:934). They are `&self` methods and the **niche
  read is assign-site/statement-only** today — the crux of O3/A3.
- `crates/regseq-model/src/const_value.rs` (~:18) — **no** symbolic-macro const
  variant (Codex-confirmed); and lowered returns emit `_0`/`return;`
  (`body_emit.rs` ~:2061) — hence hand-emit, not a synthesized body.
- `crates/codegen-coverage-runner/fixtures/b4_enums/golden.c` — the **faithful
  emitted style**: `input_Color` (a tag-only fieldless-enum struct, the
  `RegseqError` shape) and `input_n_probe` (the niche `memcpy`-into-temp read).
- Demand-driven helper machinery — commit `21f7842` (`HelperUsage`/`helper_block`):
  route the `<errno.h>` include through it (O4 recommendation), not adjacent to the
  adapter.
- `crates/regseq-backends/src/generic_c/naming.rs` — `generate_main_c` (int shim
  for `ErrResult`, void shim otherwise).
- C02 table fn from regseq-backends-26; entry classifier from regseq-backends-27.
- §13 contract: `docs/discussions/regseq-model/generic-c-backend.md` §13.

## Implementation Notes

- **O3 spike first** (the design step): a tiny hand-written C compile/run proof of
  the two reads, then decide the factoring — **recommended: factor §13 extraction
  into a backend-local, statement-producing helper** the hand-emitted adapter
  calls. *Not* a synthesized model body (can't emit symbolic consts); *not* a pure
  expression helper (niche read is statement-shaped). Confirm the chosen factoring
  leaves the `b4_enums` golden unperturbed.
- Adapter: Ok → `return 0;`; Err → `switch` the `RegseqError` discriminant →
  `return <SYMBOL>;` for each **known** variant (C02), `default: return EIO;` as
  **corrupt-discriminant defense only**. An unknown marked variant → **fail emit**
  (via the C02 `None`), never `EIO`.
- `<errno.h>` is **demand-driven** — emitted only when the adapter is.

## Non-Goals

- Not the `expected.exit` oracle (regseq-backends-29) or fixtures
  (regseq-backends-30).
- Not the `--emit bin` integration (regseq-driver-90).
- No new layout logic — reads use §13 only.

## Test Plan

1. O3 compile/run proof (throwaway) feeding the design.
2. Backend unit test: an `ErrResult` entry emits the adapter + int shim matching
   **C03**; `Ok` path returns 0.
3. **Exhaustive adapter-switch coverage** (the adapter-switch half of PLAN T10):
   assert the emitted `switch` produces the correct `return <SYMBOL>;` arm for **all
   9** RegseqError variants (reusing the regseq-backends-26 C02 table) **plus** the
   `Ok → 0` arm — so the full table is verified through the adapter, cheaply, without
   ~10 near-duplicate e2e fixtures (**D07**).
4. `--bless` then re-run: goldens byte-stable. Assert `b4_enums` **and** every
   unit-void golden are **unchanged** (**I02**).
5. (The EINVAL e2e run is completed in regseq-backends-30 once the oracle exists.)

## Quality Gate

`cargo fmt --all -- --check`; scoped clippy (`regseq-backends`,
`codegen-coverage-runner`); `RUST_TEST_THREADS=1 cargo test -p regseq-backends
-p codegen-coverage-runner -- --test-threads=1`; ledger byte-stable (runner twice
+ diff).

## Acceptance Criteria

1. `ErrResult` entry → hand-emitted adapter + int shim per **C03**; `Ok` → exit 0
   (**R01/R03/R04**).
2. Emitted codes are **symbolic** (`return EINVAL;`); `<errno.h>` demand-driven;
   no host numeric in the `.c` (**I03/R07**, **C02**).
3. `default: EIO` is defense-only; unknown marked variant **fails emit**
   (**I05/R08**).
4. The emitted adapter switch is exhaustively tested over **all 9 variants + `Ok`**
   (adapter-switch half of **T10**), reusing the C02 table (**C02/D07**).
5. Unit-void + `b4_enums` goldens **byte-identical** (**I02**); reads use §13
   (**A3**).
6. fmt + scoped clippy clean; tests green; ledger byte-stable.
