# regseq-backends-11: Generic-C instance-symbol uniqueness + linkage-aware naming (d1 follow-up)

## Description

Fix a **soundness hole surfaced by the d1 assessment** (`regseq-backends-10`,
`4a8e655`): the generic-C backend emits an instance's C symbol as
`naming::sanitize(model.strings.resolve(instance.emission_name))` — sanitise
only, with **no generic-args disambiguation and no clash net** (`mod.rs:742`
`emission_ident`, plus three inline copies in `body_emit.rs:621/711/932`). The
bridge mints `Instance.emission_name = model.items[item].def_path.rendered`
at every instance-construction site (`bridge/mod.rs:1892/1995/2128/2195/2417/
2598/2688`, `const_.rs:222`, `body/terminator.rs:989`, `body/materialise.rs:786`,
`synth/vtable.rs:320/377/404` — non-exhaustive),
which is keyed by the **`ItemId`** (the generic *definition*), so **all
monomorphizations of one generic item share the same `emission_name`**. A
`Linkage::Default` generic function instantiated at two types (e.g.
`fn id<T>(x: T) -> T` used as `id::<u32>` and `id::<i64>`) therefore emits **two
C functions with the same identifier** → a duplicate-symbol compile error (or, if
the bridge ever shares a name across distinct bodies, a silently wrong link).

The d1 spec told the implementer "`emission_name` (uniqueness guaranteed); the
backend does not re-mint" (`generic-c-backend.md` §7) — a **wrong premise**: the
bridge does not guarantee per-instance uniqueness, and the model performs no
emission-name uniqueness validation (`model.rs:3495` checks only StringId
validity). The implementer followed the spec faithfully (sanitise was a
necessary, correct addition for C-validity). The d1 fixtures are all
`#[no_mangle]` non-generic functions, so the collision is **not exercised** by
any green test — a latent hole ([[assessment-soundness-pass]]: green functional
tests can hide a load-bearing soundness gap; confirmed REAL_BUG by an external
Codex soundness pass during the d1 close).

The **type-name** path already solved the identical problem: it derives
`sanitize(DefPath.rendered) + generic_suffix(ArgsKey)` and disambiguates with a
`_clash_<n>` net (`naming.rs:52-68`, `mod.rs:422-436`). This task brings
**instance** symbols to the same standard — linkage-aware, generic-disambiguated,
clash-netted — and corrects the §7 spec premise.

### Scope boundaries (flagged for review — see § Review)

1. **Backend-side fix only.** The bridge's `emission_name` minting is **not**
   changed (out of scope; it is a shared producer surface, and the rendered
   `DefPath` is the right *base*). The fix derives the final C symbol in the
   backend, exactly as type names are derived.
2. **Linkage-aware.** **External** linkage (`NoMangle` / `export_name` /
   `Foreign`) carries an **ABI contract** — the symbol must be the verbatim
   `emission_name` (other TUs / the harness link by that exact name; sanitising
   or suffixing it would break linkage). **Internal** linkage (`Default` /
   `Weak`) symbols are TU-local → derived `sanitize(emission_name) +
   generic_suffix(instance.generic_args)` and clash-disambiguated. (Phase-1
   assumes a verbatim external symbol is already a valid C identifier; a
   non-C-identifier `export_name` is a rare exotic case — note it, do not
   special-case it here.)
3. **No new lowering.** No bank-4/5 *content* change beyond the symbol the
   prototype / definition / call / `&fn` / `Reloc` site uses; no d2 constructs;
   no change to type-name derivation, discovery, or the §13/§14 emitters.

## Input Context

### The bug (HEAD-confirmed)

- `crates/regseq-backends/src/generic_c/mod.rs:736-744` — `emission_ident` =
  `naming::sanitize(model.strings.resolve(instance.emission_name))`; its
  doc-comment asserts the (false) "this is not a re-mint … uniqueness" premise.
  **`emission_ident` is the shared helper called by the bank-4 function
  prototype + the bank-4 `extern` static declaration (`mod.rs:792`) and the
  bank-5 function definition + the bank-5 static definition (`mod.rs:859`)** —
  so routing `emission_ident` through the table covers **functions and
  statics** uniformly (consts materialise at use sites, §17, no standalone
  symbol).
- `crates/regseq-backends/src/generic_c/body_emit.rs:621` (`ReifyFnPointer` /
  `&fn`), `:711` (`Call` `Direct` callee), `:932` (`Const::Reloc` →
  `&<symbol>`) — three more sites doing the same inline `sanitize(emission_name)`.
  **Every instance-symbol site (the bank-4/5 emitters via `emission_ident` + these
  three body sites) must agree on the final symbol**, so the fix routes them all
  through one shared table (a call to / `&` of instance X must spell X's
  *definition* name).
- `crates/regseq-model/src/instance.rs:128-145` — `Instance { item: ItemId,
  generic_args: GenericArgs, emission_name: StringId, kind }`: monomorphizations
  are distinct `Instance`s sharing one `item` with different `generic_args`.
- Bridge mint sites (read-only; **not changed**; non-exhaustive):
  `crates/regseq-driver/src/bridge/mod.rs:1892/1995/2128/2195/2417/2598/2688`,
  `const_.rs:222`, `body/terminator.rs:989`, `body/materialise.rs:786`,
  `synth/vtable.rs:320/377/404` — every site sets `emission_name =
  items[item].def_path.rendered` (generic-agnostic).
- `crates/regseq-model/src/model.rs:3495` — the only emission-name validation is
  `check_string_id` (validity, **not** uniqueness).
- `crates/regseq-model/src/def_path.rs:111-114` — `DefPath { rendered: StringId,
  … }` is a property of the *item*, not the monomorphization.

### The precedent to mirror — type-name derivation

- `crates/regseq-backends/src/generic_c/naming.rs`:
  - `sanitize(s)` (19-30) — `::`→`_`, other non-`[A-Za-z0-9_]`→`_`.
  - `generic_suffix(model, args)` (36-43) — **empty for non-generic** (`args`
    empty), else `__<ArgsKey:x>` (the canonical interner key; post-mono-unique,
    build-stable). Currently **private**; make it `pub(crate)` (or wrap it).
  - `aggregate_base_name` (52-68) — the `sanitize(rendered) +
    generic_suffix(args)` pattern the instance symbol mirrors.
- `crates/regseq-backends/src/generic_c/mod.rs`:
  - `NameTable` (75-92) + `build_name_table` (422-436) — the type-tag clash net
    (`_clash_<n>`). The instance-symbol net mirrors its shape but is a
    **separate** table (C ordinary-identifier namespace ≠ struct/union/enum tag
    namespace, so instance symbols and `struct <T>` tags do **not** collide;
    instance symbols collide only with each other **and** the prelude's
    ordinary-namespace `regseq_*` helpers).
  - `emit_generic_c` (121-132) — where `names` is built once and threaded into
    banks 2–5; the new instance-symbol table is built + threaded here the same
    way.
  - `bank4_declarations` (762), `bank5_definitions` (825), the prototype/
    definition emitters (676+) — symbol-emission sites that take the new table.
- `crates/regseq-backends/src/generic_c/body_emit.rs:50/857/870/921` —
  `emit_body` + helpers take `names: &NameTable`; they gain the instance-symbol
  table param.

### Prelude ordinary-namespace identifiers to reserve

`crates/regseq-backends/src/generic_c/prelude.rs` — the clash net must reserve
the prelude's **function** identifiers (ordinary namespace) so a derived
internal symbol cannot shadow them: `regseq_panic` / `regseq_unreachable`
(48-57), `regseq_{add,sub,mul}_<T>` (142-156), `regseq_<fprefix>_to_<T>`
(160-179). (`struct regseq_overflow_<T>` and `struct rust_str` are **tags**, a
different namespace — not reserved here. libc names like `memcpy` / `printf` /
`abort` are an accepted low-risk edge — a `Default` Rust item rendering to
exactly one of those is implausible; if it ever happens the duplicate fails the
compile loudly, not silently.)

### Prior tasks

- `regseq-backends-10` (d1, done, `4a8e655`) — introduced bank-4/5 instance
  emission (the first consumer of `emission_name` as a C symbol), surfacing this
  hole. This task is its **follow-up** (the 07→09 pattern: close the parent,
  file the newly-found issue separately).
- `regseq-backends-06` (c1) — established the type-name `sanitize +
  generic_suffix` + `_clash_<n>` machinery this task reuses for instances.

### Test surface

- `crates/regseq-backends/src/generic_c/mod_tests.rs` — hand-built-model tests;
  add the generic-collision regression + the linkage cases here.
- `crates/codegen-coverage-runner/tests/generic_c_bodies.rs` — the d1 behavioral
  harness; the existing tests must stay green (their entry symbols are emitted
  consistently through the new table).

## Implementation Notes

### Change A: `naming::instance_symbol_base` (linkage-aware base name)

Add `pub(crate) fn instance_symbol_base(model, instance) -> String` to
`naming.rs` (and make `generic_suffix` `pub(crate)`), returning the
**pre-clash** symbol:

- `Linkage::NoMangle` / `export_name` / `Foreign` → the **verbatim**
  `model.strings.resolve(instance.emission_name)` (the ABI symbol; no sanitise,
  no suffix). Read the linkage from `model.items[instance.item].linkage`
  (`item.rs:138-147`).
- `Linkage::Default` / `Weak` → `sanitize(emission_name) +
  generic_suffix(model, &instance.generic_args)` (non-generic → just
  `sanitize(emission_name)`; generic monos → distinct `__<ArgsKey>` suffixes).

### Change B: the instance-symbol table + clash net (`generic_c/mod.rs`)

Add an `InstanceSymbols` table (parallel to `NameTable`) mapping `InstanceId →
final C symbol`, built once in `emit_generic_c` and threaded into banks 4/5 +
`body_emit`. **Only `InstanceKind::Function` and `InstanceKind::Static`
instances emit an ordinary C symbol** (banks 4/5, `mod.rs:776-804`/`839-870`);
`Const` instances materialise at use sites (§17, no standalone symbol) and the
`Struct`/`Enum`/`Union` instances are type *tags*, a different namespace — so the
table is built over (and the `used` set reserves) **only function/static
instances**, not consts or aggregates (avoid over-reserving names). Construction
(deterministic, `InstanceId` order):

1. **Reserve** the verbatim external symbols (every `NoMangle` / `export_name` /
   `Foreign` instance's base name) **and** the prelude ordinary-namespace
   identifiers (the `regseq_*` helpers above) into the `used` set first — these
   cannot be renamed.
2. For each instance in `InstanceId` order: take its `instance_symbol_base`; for
   an internal (`Default` / `Weak`) base already in `used`, `_clash_<n>`-
   disambiguate (the `build_name_table` algorithm, `mod.rs:422-436`); insert.
   External (verbatim) bases are taken as-is (already reserved; a verbatim/
   verbatim clash is a genuine user/ABI error — surface it, do not silently
   rename a second verbatim symbol — but two *distinct* external instances with
   the same ABI name is itself model corruption, so a hard error is acceptable).

Expose `syms.symbol(instance_id) -> &str`. The map is a pure function of the
model → byte-stable.

### Change C: route every instance-symbol site through the table

Replace the inline `naming::sanitize(... emission_name)` at `mod.rs:742`
(`emission_ident` → look up `syms.symbol(id)`) — this single helper change
covers the bank-4 function prototype + `extern` static decl (`mod.rs:792`) and
the bank-5 function definition + static definition (`mod.rs:859`), since they
all call `emission_ident`. Then the three `body_emit.rs:621/711/932` sites
(thread the table; look up `syms.symbol(*inst)` / the resolved `Reloc` target
id). Thread `syms: &InstanceSymbols` through `bank4_declarations`,
`bank5_definitions`, the prototype/definition emitters, and `emit_body` + its
helpers — the same way `names: &NameTable` is already threaded. The result:
**functions and statics** both get a clash-disambiguated, linkage-aware symbol,
and every reference (prototype / definition / call / `&fn` / `Reloc`) spells the
same one.

### Change D: correct the §7 spec premise

In `docs/discussions/regseq-model/generic-c-backend.md` §7 (the "Identifier
emission policy" paragraph, ~lines 285-305), replace the "**bridge-minted
`Instance.emission_name` (uniqueness guaranteed); the backend does not re-mint**"
sentence with the accurate rule: `emission_name` is the bridge-minted,
**generic-agnostic, possibly-not-C-valid `DefPath.rendered`**; the backend
**derives** the final C symbol — **verbatim** for external linkage
(`NoMangle` / `export_name` / `Foreign`, an ABI contract) and `sanitize(rendered)
+ generic_suffix(generic_args)` with a `_clash_<n>` net for internal
(`Default` / `Weak`) — exactly mirroring the type-name derivation. (Flag this as
a canonical-doc fix, the way c1/c2 flagged the §14/§15/§13 doc corrections.)

§17's shim/vtable shorthand "slot `i` is initialised to `&<shim emission_name>`"
(`generic-c-backend.md` ~lines 1149-1153) uses `emission_name` loosely for the
same final-symbol concept; add a one-line note there that the corrected §7
final-symbol derivation governs vtable slot references too (the actual `&<shim>`
emission lands with the deferred `dyn`/vtable task, but the wording should not
re-assert the stale "verbatim emission_name" premise).

### Change E: tests (`mod_tests.rs` + verify the runner tests)

- **Regression (the bug):** a hand-built model with **two `Function` instances
  sharing one `item`, different `generic_args`, `Default` linkage** → the two
  prototypes/definitions get **distinct** C symbols (`…__<hexA>` / `…__<hexB>`),
  and the emitted TU **compiles** (no duplicate symbol). Without the fix this is
  a duplicate-symbol collision.
- **Linkage cases:** a `NoMangle` instance → **verbatim** symbol (no suffix, no
  sanitise-mangling); a `Default` **non-generic** instance → `sanitize(rendered)`
  (no suffix); a generic `Default` mono → suffixed.
- **Call/`&fn`/`Reloc` consistency:** a call to (and `&fn` / `Reloc` of) a
  generic instance spells the **same** disambiguated symbol as its definition.
- **Determinism:** `emit_generic_c` byte-identical across two calls on a model
  with generic instances.
- **Regression-free:** the d1 behavioral tests (`generic_c_bodies.rs`) stay
  green (NoMangle entry symbols unchanged through the table).

## Non-Goals

- **No bridge change** — `emission_name` minting stays `DefPath.rendered`; the
  fix is backend-side derivation.
- **No type-name / discovery / §13 / §14 change** — type tags already correct.
- **No d2 constructs**; no new bank-4/5 content beyond the symbol used.
- **No exotic-`export_name` handling** (a non-C-identifier ABI name) — Phase-1
  assumes external symbols are valid C identifiers; note it, don't special-case.
- **No libc-namespace reservation** beyond the prelude `regseq_*` helpers
  (accepted low-risk edge; fails loud if ever hit).
- **No deps; no harness / model / matrix / ledger / `test-ir-fixtures` change.**
  Confined to `crates/regseq-backends/src/generic_c/**`, the §7 doc fix, and the
  `tasks.json` registration.

## Test Plan

### B1: build + workspace green
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`
  green — **all d1 + c1/c2 tests still pass** (NoMangle/verbatim symbols
  unchanged) + the new uniqueness tests.

### B2: the collision is fixed
- The two-mono `Default` generic-function model emits distinct symbols + the TU
  compiles clean (Change E regression). A pre-fix snapshot of the same model
  would emit a duplicate symbol — assert the post-fix symbols differ.

### B3: linkage + consistency + determinism
- Verbatim external, suffixed internal, call/`&fn`/`Reloc` ↔ definition symbol
  agreement, byte-stability (Change E).

### B4: fmt + clippy + diff scope
- `cargo fmt --all -- --check` + `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean.
- Diff confined to `crates/regseq-backends/src/generic_c/{naming.rs, mod.rs,
  body_emit.rs}` + `mod_tests.rs`, the `generic-c-backend.md` §7 fix, and
  `tasks.json`. No bridge / model / harness / ledger change.

## Review

### Files to send to Codex
This spec; `docs/discussions/regseq-model/generic-c-backend.md` §7 (+ §17);
`crates/regseq-backends/src/generic_c/{naming.rs, mod.rs, body_emit.rs,
prelude.rs}`; `crates/regseq-model/src/{instance.rs, item.rs, def_path.rs,
model.rs (~3495)}`; the bridge mint sites
(`crates/regseq-driver/src/bridge/mod.rs:1892/1995/2128/2417`, `const_.rs:222`,
`body/terminator.rs:989`); `crates/codegen-coverage-runner/tests/generic_c_bodies.rs`.

### Reviewer focus
- **Does the fix close the collision** for `Default` generic monomorphizations
  (distinct `generic_suffix(ArgsKey)`), and is `generic_suffix` keyed on the
  **instance's** `generic_args` (not the item's)?
- **Linkage correctness**: external (`NoMangle`/`export_name`/`Foreign`)
  verbatim (ABI contract preserved); internal (`Default`/`Weak`) derived +
  clash-netted. Are namespaces right (instance symbols vs `struct` tags don't
  collide; prelude `regseq_*` reserved; libc edge acceptably deferred)?
- **Every instance-symbol site** — `emission_ident` (`mod.rs:742`, used by the
  bank-4/5 function-prototype/definition **and** static decl/def at `mod.rs:792`/
  `:859`) + `body_emit.rs:621/711/932` — routes through the one table, so a
  call/`&fn`/`Reloc` (and a static reference) spells the definition's symbol.
  **Functions and statics both covered.**
- **Determinism** of the table (instance order + clash net) and **regression
  safety** (d1/c1/c2 tests green; verbatim entry symbols unchanged).
- **Spec fix accuracy** (§7) and whether it stays consistent with §17 linkage.
- Scope discipline (backend-only; no bridge/type-name/d2 change); citation drift.

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/11-generic-c-instance-symbol-uniqueness.md
— a d1 (regseq-backends-10) follow-up fixing a confirmed soundness hole: the
generic-C backend emits an instance C symbol as sanitize(emission_name) with no
generic-args disambiguation + no clash net (mod.rs:742, body_emit.rs:621/711/
932), but the bridge mints emission_name = items[item].def_path.rendered
(generic-agnostic, per-item), so two Default-linkage monomorphizations of one
generic fn emit the SAME C symbol → duplicate-symbol collision. The fix:
linkage-aware instance naming (NoMangle/export_name/Foreign verbatim;
Default/Weak = sanitize(rendered) + generic_suffix(instance.generic_args)) via a
clash-netted instance-symbol table threaded through banks 4/5 + body_emit,
mirroring the type-name path (naming.rs:52-68, mod.rs:422-436); + a §7 spec
correction (emission_name is NOT unique/C-valid). Backend-only; no bridge change.
Identify:
- Whether the fix closes the collision (generic_suffix on the INSTANCE's
  generic_args) and any residual collision (cross-item sanitize collision;
  verbatim-vs-derived; prelude regseq_* shadowing; libc edge).
- Linkage correctness + C namespace reasoning (instance symbols vs struct tags).
- All 4 symbol sites routed through one table (call/&fn/Reloc ↔ definition).
- Determinism; regression safety (d1/c1/c2 green; verbatim entries unchanged).
- §7 spec-fix accuracy vs §17; scope discipline (no bridge/type-name/d2 change);
  citation drift vs HEAD.
Read: generic-c-backend.md §7/§17; regseq-backends/src/generic_c/{naming,mod,
body_emit,prelude}.rs; regseq-model/src/{instance,item,def_path,model}.rs; the
bridge mint sites (bridge/mod.rs:1892/1995/2128/2417, const_.rs:222,
body/terminator.rs:989); codegen-coverage-runner/tests/generic_c_bodies.rs.
</task>

<grounding_rules>
- Cite file:line for every claim. Mark hypotheses as hypotheses.
- Don't speculate about code/spec you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
CLOSES-COLLISION (generic_suffix on instance args; residuals): PASS / FAIL with citation
LINKAGE-AND-NAMESPACE (verbatim external / derived internal; tags vs ordinary; prelude reserved): PASS / FAIL with citation
ALL-SITES-ROUTED (mod.rs:742 + body_emit 621/711/932 via one table): PASS / FAIL with citation
DETERMINISM-AND-REGRESSION (byte-stable; d1/c1/c2 green; verbatim unchanged): PASS / FAIL with citation
SPEC-FIX (§7 accurate; consistent with §17): PASS / FAIL with citation
SCOPE-DISCIPLINE (backend-only; no bridge/type-name/d2): PASS / FAIL with citation
CITATION DRIFT: list or "none"
NEW ISSUES: numbered or "none"
REQUIRED FIXES: numbered (only if VERDICT != APPROVED)
</structured_output_contract>

<default_follow_through_policy>
Do not ask clarifying questions. Read the files, decide, stop.
</default_follow_through_policy>
```

## Quality Gate
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `cargo fmt --all -- --check` + `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean.
- `cargo test -p regseq-backends -p codegen-coverage-runner -- --test-threads=1`
  green (d1/c1/c2 unchanged + the new uniqueness/linkage tests).
- The two-mono generic model emits distinct, compiling symbols; verbatim
  external symbols unchanged; `emit_generic_c` byte-stable.
- `git diff --stat` confined to the Change-A–E file set + §7 doc fix + registration.
- Codex Task Review: APPROVED or APPROVED_WITH_MINOR_FIXES.

## Acceptance Criteria

1. **Collision fixed**: a `Default`-linkage generic function instantiated at ≥2
   types emits ≥2 **distinct** C symbols (`sanitize(rendered) +
   generic_suffix(instance.generic_args)`), and the TU compiles.
2. **Linkage-aware**: `NoMangle` / `export_name` / `Foreign` symbols are
   **verbatim** `emission_name` (ABI contract); `Default` / `Weak` are derived +
   `_clash_<n>` disambiguated against a reserved set (verbatim external names +
   prelude `regseq_*` helpers).
3. **One table, all sites (functions + statics)**: an `InstanceSymbols` table
   built once in `emit_generic_c`, threaded through banks 4/5 + `body_emit`;
   `emission_ident` (`mod.rs:742`, covering the bank-4/5 fn prototype/definition
   **and** static decl/def at `mod.rs:792`/`:859`) + `body_emit.rs:621/711/932`
   all look up the same symbol (call/`&fn`/`Reloc` + static refs match the
   definition).
4. **Determinism**: `emit_generic_c` byte-identical across two calls on a
   generic-instance model.
5. **§7 spec corrected**: the "emission_name uniqueness guaranteed / no re-mint"
   premise replaced with the linkage-aware derived-symbol rule, consistent with
   §17.
6. **No regression / no scope creep**: d1 + c1/c2 tests green; no bridge /
   type-name / discovery / §13/§14 / harness / model change; diff confined to the
   Quality-Gate file set.
