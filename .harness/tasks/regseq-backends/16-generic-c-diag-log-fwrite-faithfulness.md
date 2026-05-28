# regseq-backends-16: Generic-C diag-log **byte-faithful `fwrite` emission** — fix the interior-NUL `&str` silent truncation (§8.1/§8.2/§8.3 spec delta) (Phase-1 task `&str`-NUL)

## Description

Task e (`regseq-backends-15`, `567e6e3`) lowered every diag-log call to a **single
NUL-terminated `printf(fmt, args)`** (§8.1 "single emitter"). The mandatory post-close
Codex soundness pass (the 5th real catch) found a **latent silent miscompile**: a Rust
`&str` is **length-prefixed and may contain an interior `\0`** (valid UTF-8), but the
emission routes every byte through C's **NUL-terminated** constructs — the `printf`
format string and `%s` — so an interior NUL **silently truncates** the output. Verified
(task-e assessment): `info!("pre\0post={}", 7u32)` →
`printf("[INFO] pre\000post=%u\n", …)` → prints only `[INFO] pre`. Three paths carry the
bug: the **format-template literal**, a **static `&str` value**, and a **non-static
runtime `&str` value** (the last not statically detectable). Task e is correct for all
NUL-free text — the first-green-ledger milestone stands — but the truncation is live in
the tree.

This task is the architect-chosen **full length-aware redesign** (not a fail-loud
interim): replace the single `printf` with a **uniform `fwrite`-based emission** in which
**every byte that reaches stdout is written by `fwrite(ptr, 1, n, stdout)` with an
explicit byte count `n`**, so all content — including interior NUL — is reproduced
faithfully:

- every **literal run** (the bracketed severity tag, the author template's literal text,
  and the trailing `\n`) → `fwrite("<bytes>", 1, <decoded-byte-len>, stdout)`;
- every **numeric conversion** → rendered into a fixed stack buffer with `snprintf`
  (reusing task-e's §8.2/§26.7 specifier + cast mapping **unchanged**), then `fwrite`n the
  returned length (numeric output is NUL-free; the buffer is sized so `snprintf` provably
  never truncates — see Change C);
- every **`&str` value** → `fwrite` of its full byte range: a **static** literal via its
  CMS bytes + known length (a C string literal + explicit count); a **non-static** value
  via the §3.2 fat pointer's `.data` / `.len`.

This **deletes** the §8.3 stack-buffer + cap-256 + deterministic-truncation contract
entirely: with `fwrite((op).data, 1, (op).len, stdout)` there is **no buffer, no cap, no
truncation** — a `&str` of any length prints in full. It is a deliberate **§8.1/§8.2/§8.3
canonical-spec delta** (`generic-c-backend.md`), the whole point of the task
([[diag-log-str-nul-faithfulness]]).

### Three pieces (all in scope)

1. **Backend redesign** (`body_emit.rs`): rewrite `emit_diag_log` (`:1961`) to the
   uniform-`fwrite` statement sequence; reuse `parse_template` / `diag_specifier_and_cast_for`
   / the static-vs-non-static `&str` discriminator (`read_static_str_bytes` /
   `is_str_ref_place`) **unchanged**; drop the now-dead non-static stack buffer + the
   now-dead `c_escape` `double_percent` parameter.
2. **Spec delta** (`docs/discussions/regseq-model/generic-c-backend.md` §8): §8.1
   single-emitter → uniform-`fwrite`; §8.2 the `Ref(Str)` row; §8.3 retitled + rewritten
   to byte-faithful `fwrite` (the truncation contract removed); a §8.4 rationale note.
3. **Fixtures + ledger**: **re-bless all** existing diag goldens (every emission shape
   changes); **repurpose** `diag_str_truncation` → `diag_str_long` (a ≥256-byte non-static
   `&str` now prints **in full**); **add `diag_nul`** (the interior-NUL regression:
   template + static + non-static `&str`, `expected.stdout` carrying literal NUL bytes);
   regenerate the byte-stable `coverage_ledger.{json,md}`.

### Explicitly NOT in scope

- **No `spec.md` change EXCEPT the narrow generic-C-binding-prose corrections the redesign
  falsifies.** The IR producer-surface proper is unchanged — the template grammar (§26.7),
  the DiagLogPeel/translation contract (§26.6), the D-rule **triggers and values** (D8's
  `max_str_args` stays bridge-side; the IR always permitted interior NUL in a `&str`), and
  the **Pipeline-A prohibition** (no `LogArg` reconstruction) are all untouched. But **four**
  spots **restate** the generic-C target's now-removed stack-buffer/`printf`/NUL-termination
  mechanism and become **stale normative prose** after this §8 delta — they must be
  corrected for doc consistency (a soundness fix cannot leave contradictory normative text):
  **§26.1** ("the generic-C target's `printf` with leading severity tag", `:12420`),
  **§26.8 D8** ("Bounds per-call stack use against the per-`%s`-site temporary-buffer
  allocation", `:12894`), **§26.9** ("hosted userspace, libc, `printf`", `:12930`), and
  **§26.10** ("temporary stack-buffer fallback for non-static `&str`", `:13005-13013`). The
  edits are tightly scoped to those generic-C clauses (Change E). (The standing
  §26.7-vs-bridge hex-drift item is unrelated and stays open.)
- **No bridge / model change.** DiagLogPeel and the `&str` carrier shape are unchanged;
  the backend reads the same flat peeled operands (Pipeline-A, §8.5). DL-5 / D8
  (`max_str_args`, bridge-side, currently `None`/unenforced, `peel.rs:179`) stays as-is.
- **No prelude change.** `fwrite` / `snprintf` are in `<stdio.h>`, already
  `#include`d (`prelude.rs:30-35`). `<string.h>`/`memcpy` stay (used pervasively by other
  emitters — the dropped non-static-`&str` `memcpy` is not the only caller).
- **No Doc-1 matrix `status`-column change** — coverage green-ness lives in the **ledger**
  (regenerated + committed). The matrix stays stable.
- **No new `EmitError` variant**, no change to the specifier/cast mapping, the parser, or
  the static-vs-non-static discriminator (all reused verbatim).

## Input Context

### Design source — `docs/discussions/regseq-model/generic-c-backend.md` §1 + §7.1 + §8 (the delta target)

- **§1 Mission and scope** (`:30`): the phase-1 scope line "hosted userspace, libc, `printf`,
  `main()`, …" names `printf` — stale after the redesign. Reword "libc, `printf`" → "libc
  stdio" (Change E); nothing else in §1 changes.
- **§7.1 `Call` terminator rule** (359–365): the diag-log interception clause says "emit the
  §8 **`printf`** instead of a call" (`:363`) — a same-doc cross-reference that the redesign
  falsifies. Reword to "emit the §8 diagnostic-logging stdio emission sequence instead of a
  call" (Change E); the predicate + `goto` + no-C-definition parts are unchanged.
- **§8.1 Per-`LogLevel` C call shape** (396–409): currently "Single emitter (`printf`)";
  the per-level **bracketed tags** (`[ERROR]`/`[WARN]`/`[INFO]`/`[DEBUG]`) are **retained**.
  The delta replaces the single-`printf` mechanism with the uniform-`fwrite` statement
  sequence (Change E).
- **§8.2 Per-arg C specifier** (411–425): the numeric rows (`%d`/`%u`/`%lld`/`%llu` per
  width, modifier composition) are **retained** (they now describe the `snprintf` format
  spec). Only the **`Ty::Ref(Ty::Str,_)` row** changes — from "`%s`" to "written
  byte-faithfully via `fwrite` (§8.3); not a printf/snprintf specifier."
- **§8.3 `&str` NUL-termination contract** (427–480): the section to **retitle + rewrite**.
  The current normative truncation contract (256-byte cap, `min(len, cap-1)` copy, NUL,
  no-marker, "truncation whenever `source.len() >= cap`") is **removed**; replaced by the
  byte-faithful `fwrite` rule (no buffer, no cap, no truncation).
- **§8.4 `max_str_args_per_call = 8`** (482–501): the **value stays** (bridge-side, §26.8
  D8, unenforced). Its **stack-budget rationale** ("`8 * cap` bytes") is now obsolete (no
  per-`&str` buffer); add a one-line note that the bound is now a bridge-side
  argument-count ceiling, not a stack bound. **Do not change the value.**
- **§8.5 Pipeline A consumption** (503–512): a **bounded** edit. Its current prose says the
  backend "emits the `printf` call with each peeled operand rendered element-by-element
  through §8.2's per-arg specifier" — stale after the redesign (numerics go through §8.2
  via `snprintf`; literals + `&str` go through `fwrite`, not a specifier). Reword the
  emission description to the uniform-`fwrite` sequence; **retain** the binding rule it
  exists to state (read the post-peel flat operands; **MUST NOT** reconstruct a `LogArg`
  slice or any pre-peel carrier — §26.9 uniformity).

### IR-side prose to correct — `docs/discussions/regseq-model/spec.md` §26 (generic-C-binding restatements only)

Four spots restate the generic-C target's now-removed mechanism; the redesign falsifies
them. The corrections are narrow (Change E); nothing else in §26 changes.

- **§26.1** (`:12414-12422`): the parenthetical example "Per-target emitter shapes (e.g.,
  the **generic-C target's `printf`** with leading severity tag)" (`:12419-12420`) → "the
  generic-C target's libc-stdio (`fwrite`) emission with leading severity tag" (only
  "`printf`" is stale; the leading-tag + the Pipeline-A flat-operand statement stay).
- **§26.8 D8** (`:12894`): the trigger + value are unchanged (`&str` operand count ≤ the
  target's `max_str_args_per_call`); only the trailing rationale "**Bounds per-call stack
  use against the per-`%s`-site temporary-buffer allocation**" is stale (no per-`%s` buffer
  post-`fwrite`) → reword to a bridge-side argument-count ceiling.
- **§26.9** (`:12922-12955`): the cross-ref "Stage 14 specs the generic-C binding — hosted
  userspace, libc, **`printf`**" (`:12930`) → "libc stdio" (generic-C now emits
  `fwrite`/`snprintf`, not `printf`). The general cross-backend **Pipeline-A uniformity
  rule** (`:12942-12955`, "the target's printf-family call"; binding on all backends) is a
  loose family descriptor — **left as-is** (not generic-C-specific; the binding *prohibition*
  is what matters and is unchanged).
- **§26.10** (`:13005-13013`): "Generic-C is pinned in `generic-c-backend.md` §8.3
  (static-literal direct emission + **temporary stack-buffer fallback for non-static
  `&str`**)" → "byte-faithful `fwrite` emission (no NUL-termination needed)". The
  "Per-target `&str` null-termination strategy choice" framing stays for phase-2 targets;
  only the **generic-C** clause changes.

### The soundness finding being fixed ([[diag-log-str-nul-faithfulness]])

A Rust `&str` (the template literal OR a `&str` value arg) is length-prefixed and may
contain an interior `\0`. Task e's emission: `c_escape` maps `\0`→`\000` into the C
**format string** (so `printf` stops at the first NUL), and `%s` stops at the first NUL in
a value. The 10 task-e green fixtures all use NUL-free text, so the bug stayed latent.
The task-e probe verified the **bridge accepts** an interior-NUL template and a NUL-bearing
`&str` (it reached emission), so all three paths are reachable in a bridged model — the
`diag_nul` fixture (Change F) is the regression guard.

### Verification flow — `docs/discussions/generic-c-backend/phase-1/03-verification-flow.md`

The 6-stage pipeline + the dual oracle (golden `.c` snapshot + golden stdout) are built
(task a) and were first exercised end-to-end by task e. The behavioral oracle
(`oracle.rs` `behavioral_stdout` → `compare_exact`) is **byte-exact** (`actual ==
expected` on the captured `String`); stdout is captured via `String::from_utf8_lossy`
(`toolchain.rs:117`). **NUL (U+0000) is valid UTF-8**, so it survives capture and the `==`
losslessly — an interior-NUL `expected.stdout` is comparable byte-exactly (Change F / B5).

### Code anchors (confirm line numbers at HEAD `6789161`)

- `crates/regseq-backends/src/generic_c/body_emit.rs` — the task-e diag-log seam:
  - `:1961` `fn emit_diag_log(level, args, target)` — the **function to rewrite** (Change
    A/B/C/D). Currently builds one `fmt` string + a `c_args` list + per-`%s` `_logstr`
    buffers, then `printf(fmt, args); goto bb_<t>;` (`:1995-2073`).
  - `:2003-2058` the segment walk: `:2008` `c_escape(bytes, true)` (literal → format
    string, `%`-doubled — **becomes `fwrite`, no doubling**); `:2018-2024` static `&str`
    → C string literal `%s` arg (**becomes a value `fwrite`**); `:2030-2043` non-static
    `&str` → the `_logstr<k>[256]` stack buffer + `memcpy` + NUL (`:2039` — **deleted**,
    replaced by `fwrite((op).data, 1, (op).len, stdout)`); `:2045-2056` numeric →
    `diag_specifier_and_cast_for` + `c_args.push(format!("{cast}({expr})"))` (**the
    spec+cast is reused; the rendering moves to `snprintf`+`fwrite`**).
  - `:2082` `read_template_bytes` / `:2094` `read_static_str_bytes` / `:2101`
    `read_str_carrier_bytes` / `:2149` `is_str_ref_place` / `:2162` `diag_numeric_ty` —
    **all reused unchanged** (the template read, the static-vs-non-static discriminator,
    the numeric-`Ty` recovery).
  - `:3709` `diag_specifier_and_cast_for(modifier, ty) -> (spec, cast)` — **reused
    verbatim** (the `snprintf` format spec + the value cast); `:3767`
    `require_default_modifier`; `:3814` `parse_template`; `:3867` `parse_placeholder`;
    `:3783` `Modifier`; `:3800` `Segment`.
  - `:3934` `c_escape(bytes, double_percent) -> String` — the `double_percent=true` arm
    (the `%`→`%%` doubling) is **orphaned** by this change (literals are `fwrite`n, not
    formatted; the `snprintf` format is the bare specifier). **Drop the parameter** →
    `c_escape(bytes) -> String` (Change D); the only `true` caller was `:2008`; the only
    other callers are `:2022` (`false`) + the `mod_tests.rs` unit test.
- IR surface (read-only — bridge certificates): `Ty::Ref(Ty::Str,_)` lowers to the §3.2
  fat struct `rust_str { const uint8_t *data; size_t len; }`; the non-static path reads
  `(<operand_expr>).data` / `.len` (the task-e buffer already used exactly these members,
  `:2039`).

### Prior task

- **task e** (`regseq-backends-15`, `567e6e3`): the §8 diag-log → `printf` lowering this
  task re-shapes; the harness bring-up (`--bless`, entry resolution) + the 10 on-disk
  fixtures this task re-blesses. The first-green-ledger milestone — DL-1/2/3/4/6/7 +
  TM-2/3/5/8 green; this task keeps them green (no row flips), fixing the latent
  truncation under them.

### Test surface

- `crates/regseq-backends/src/generic_c/mod_tests.rs`:
  - `:4985+` the "Section 8 diagnostic-logging → printf: structural assertions" block (the
    `emit_diag_call` harness): the **emission-shape assertions** (`:5370`
    `printf("[INFO] x=%u\n", (unsigned)(`; `:5387-5398` the `_logstr0[256]` / `< (256 - 1)
    ?` / `] = '\0';` / `[INFO] a=%s b=%s\n` buffer + `%s` shape) **must be rewritten** to
    the uniform-`fwrite` shape.
  - `:5132` `specifier_default_arms_reuse_the_printf_widths_and_casts` + the parser /
    specifier-cast / admissible-type tests — **kept** (the mapping is reused unchanged).
  - `:5241` `c_escape(b"50% done", true) == "50%% done"` — **removed** (param dropped); the
    `false` assertions (`:5242-5245`) stay.
- `crates/codegen-coverage-runner/tests/generic_c_bodies.rs` — add **interior-NUL**
  compile-**and-run** cases (template-NUL, static-`&str`-NUL, non-static-`&str`-NUL →
  byte-exact stdout incl. the NUL bytes); add a **long non-static `&str`** case (≥256,
  printed in full). The existing diag behavioral cases assert runtime stdout/exit (not
  emitted C), so they remain valid across the emission-shape change
  ([[golden-c-emit-process-determinism]]).
- `crates/codegen-coverage-runner/fixtures/diag_*/` — the on-disk corpus (re-blessed;
  `diag_str_truncation`→`diag_str_long`; `+ diag_nul`).

## Implementation Notes

### Change A: rewrite `emit_diag_log` to the uniform-`fwrite` statement sequence (`body_emit.rs:1961`)

Keep the front matter unchanged: the `target.ok_or(…)` fail-loud (`:1969`); read `args[0]`
via `read_template_bytes` (`:1976`); `parse_template` (`:1977`); the placeholder/arg
**arity check** (`:1980-1989`); `value_ops = &args[1..]`. Then **replace** the format-string
+ `c_args` + buffer assembly (`:1991-2073`) with the following deterministic construction.

**Canonical construction (a `pending_literal` byte accumulator).** Let
`tag = "[" ++ level_tag(level) ++ "] "` (bytes). Maintain `pending: Vec<u8>` initialised to
`tag`, an output statement list, and a per-call **numeric counter** `k = 0`. Walk the
parsed `segments` in order:

- **`Literal(bytes)`** → append `bytes` to `pending` (defer; do not emit yet). (The parser
  already coalesces adjacent literals; the tag is thus merged into the leading literal run,
  and consecutive literal text accumulates.)
- **`Placeholder(modifier)`** → first **flush** `pending` (emit a literal `fwrite`, see
  Change D) and reset `pending` to empty; then emit the placeholder's write:
  - a **`&str`** operand (`read_static_str_bytes(op)?.is_some()` → static; else
    `is_str_ref_place(op)` → non-static): Change B. `require_default_modifier(*modifier)?`
    first (a hex modifier on `&str` → `Unsupported`, defensive — unchanged).
  - else a **numeric** operand: Change C, consuming + incrementing `k`.
  (`diag_numeric_ty` / `operand_expr` recover the type / expression exactly as today.)

After the walk, **append `"\n"` to `pending`** and flush it as the **final** literal
`fwrite` (`pending` is always non-empty — it holds at least the tag and/or `\n`). Then
append `goto bb_<target>;`. No destination assignment (unit return) — unchanged.

This merges the tag into the leading literal run and the `\n` into the trailing literal
run, and emits exactly one statement per piece. Worked shapes (the canonical goldens):

- `info!("connected")` → `fwrite("[INFO] connected\n", 1, 16, stdout); goto bb_<t>;`
- `info!("{}", n_u32)` →
  `fwrite("[INFO] ", 1, 7, stdout); { char _n0[24]; int _l0 = snprintf(_n0, sizeof _n0, "%u", (unsigned)(<n>)); fwrite(_n0, 1, _l0, stdout); } fwrite("\n", 1, 1, stdout); goto bb_<t>;`
- `info!("v={:08x} s={}", v_u64, s)` (non-static `s`) →
  `fwrite("[INFO] v=", 1, 9, stdout); { char _n0[24]; int _l0 = snprintf(_n0, sizeof _n0, "%08llx", (unsigned long long)(<v>)); fwrite(_n0, 1, _l0, stdout); } fwrite(" s=", 1, 3, stdout); fwrite((<s>).data, 1, (<s>).len, stdout); fwrite("\n", 1, 1, stdout); goto bb_<t>;`

(The whitespace/line-break layout within the returned terminator string is the
implementer's choice but must be **deterministic** — the goldens capture it; match the
existing one-statement-run style.)

### Change B: the `&str` value `fwrite` (replaces the `%s` literal + the stack buffer)

Reuse the **unchanged** static-vs-non-static discriminator (`read_static_str_bytes` /
`is_str_ref_place`). Per `%s`-equivalent site:

- **Static literal** (`read_static_str_bytes(op)?` → `Some(bytes)`): emit
  `fwrite("<c_escape(bytes)>", 1, <bytes.len()>, stdout);` — the CMS bytes rendered as a C
  string literal (Change D's escaper, **no `%`-doubling** — it is not a format string) with
  the **explicit decoded byte count** `bytes.len()`. The literal's implicit trailing NUL is
  irrelevant (`fwrite` writes exactly `n` bytes); an interior NUL is rendered `\000`
  (one byte) and counted as one byte — faithful.
- **Non-static** (`is_str_ref_place(op)` → true): with `expr = self.operand_expr(op,
  None)?` (the §3.2 fat struct), emit `fwrite((<expr>).data, 1, (<expr>).len, stdout);`.
  **No buffer, no cap, no truncation, no NUL append** — `fwrite` writes exactly `.len`
  bytes from `.data` (the fat pointer guarantees `.len` valid bytes → no over-read),
  reproducing the full byte range including any interior NUL.

The task-e `_logstr<k>[256]` / `memcpy` / `min(len, 255)` / NUL prelude (`:2038-2041`) is
**deleted**.

### Change C: the numeric `snprintf` + `fwrite` block (reuses the §8.2/§26.7 mapping verbatim)

For a numeric placeholder, recover `ty = self.diag_numeric_ty(op)` and
`(spec, cast) = diag_specifier_and_cast_for(*modifier, &ty)?` **exactly as task e** (the
`ll`-for-64-bit-hex, the two-step source-width signed-hex cast, the admissible-type
whitelist — all unchanged). Then render into a fixed stack buffer and `fwrite` the
**returned length**:

```c
{ char _n<k>[24]; int _l<k> = snprintf(_n<k>, sizeof _n<k>, "<spec>", <cast>(<operand_expr>)); fwrite(_n<k>, 1, _l<k>, stdout); }
```

with `<k>` the per-call numeric counter (→ independent block-scoped names `_n0`, `_n1`, …;
the enclosing `{ }` also isolates them). The `<spec>` is the bare specifier (e.g. `%u`,
`%08llx`, `%lld`) — no literal text, no `%`-doubling.

**Buffer-size soundness (load-bearing — the `snprintf`/`fwrite` invariant).** `snprintf`
returns the number of characters it *would* have written (excluding the NUL); if that
exceeded the buffer, `fwrite(_n<k>, 1, _l<k>, stdout)` would **over-read**. The buffer must
therefore be **provably larger than the maximum admissible numeric rendering**:

- the widest admissible output is **20 bytes** — `%lld` of `i64::MIN`
  (`-9223372036854775808`) or `%llu` of `u64::MAX` (`18446744073709551615`); any hex form
  is ≤ 18 (`0x` + 16 digits; the widest width modifier `#010` = 10 < 18 adds nothing); the
  default C locale has no digit grouping; `Bool`→`%d` is 1 byte; `Isize`/`Usize` are
  **non-admissible** (rejected by the whitelist, so never reach `snprintf`).
- a **24-byte** buffer (≥ 20 + NUL = 21) therefore guarantees `snprintf` **never
  truncates** → `_l<k>` is the exact, non-negative length ≤ 20 < 24 → `fwrite` reads only
  in-bounds bytes. No clamp is needed (the truncation case is unreachable — no defensive
  handling for an impossible scenario).

### Change D: the literal-run `fwrite` + the `c_escape` parameter removal

**Literal flush.** Emitting a non-empty `pending` literal run (incl. the tag and/or `\n`):
`fwrite("<c_escape(pending)>", 1, <pending.len()>, stdout);`. The count is the **decoded
byte length** `pending.len()` — **not** the escaped C-literal length (e.g. the literal
`line1\nline2=` is **12** bytes; the C literal `"line1\nline2="` renders `\n` as one byte;
pass `12`). An interior NUL in template literal text is `\000` in the C literal (one byte)
and counted once — faithful.

**`c_escape` orphan removal.** The escaper's `%`→`%%` doubling (`double_percent=true`) was
needed only because literal runs went into a `printf` **format** string. Now literals are
`fwrite`n raw and the numeric `snprintf` format is the bare specifier — so **no caller
needs `double_percent=true`**. Drop the parameter: `pub(crate) fn c_escape(bytes: &[u8]) ->
String` (remove the `b'%' if double_percent => "%%"` arm); update the two body-emit call
sites (literal flush + static-`&str` value) and remove the `mod_tests.rs:5241`
`true`-arm assertion (keep the `false` assertions). (If review prefers minimal churn the
parameter may be retained and always passed `false`; removal is the surgical choice per
the orphan-cleanup rule.)

### Change E: the canonical-spec delta (`generic-c-backend.md` §8 + the narrow `spec.md` §26 generic-C-binding corrections)

A bounded, deliberate §8 rewrite (the task's stated purpose — distinct from task e, which
shipped no Doc edit) **plus** two one-line same-doc cross-ref fixes (§1, §7.1) **plus** the
narrow `spec.md` §26 corrections for the generic-C-binding restatements the redesign
falsifies:

- **§1 Mission and scope** (`generic-c-backend.md:30`) — the phase-1 scope line "hosted
  userspace, libc, **`printf`**, `main()`, …" → "hosted userspace, libc stdio, `main()`, …"
  (only "`printf`" is stale; the generic-C target now emits `fwrite`/`snprintf`).
- **§7.1** (`generic-c-backend.md:359-365`, the `Call` terminator rule) — the diag-log
  interception clause says "emit the §8 **`printf`** instead of a call" (`:363`); reword to
  "emit the §8 diagnostic-logging stdio emission sequence instead of a call". Same-doc
  cross-reference only; the interception predicate + `goto bb_<target>` + no-C-definition
  parts are unchanged.
- **§8.1** — replace "Single emitter (`printf`) … The format string emitted to the C code
  is the concatenation …" with the **uniform-`fwrite`** mechanism: a single output stream
  (stdout); each call lowers to a deterministic statement sequence in which every literal
  run (the bracketed tag, the template's literal text, the trailing `\n`) and every `&str`
  value is written with `fwrite(ptr, 1, n, stdout)` (an explicit byte count → interior NUL
  faithful), and every numeric conversion is rendered with `snprintf` into a fixed stack
  buffer then `fwrite`n. State the **rationale**: a NUL-terminated `printf` format string
  + `%s` silently truncate interior NUL in length-prefixed Rust `&str`/literals. Retain the
  per-level tag table.
- **§8.2** — change only the `Ty::Ref(Ty::Str,_)` row to "byte-faithful `fwrite` (§8.3); no
  `printf`/`snprintf` specifier"; the numeric rows + modifier composition are unchanged
  (now describing the `snprintf` format spec).
- **§8.3** — retitle to **"`&str` byte-faithful emission"** and rewrite: both static and
  non-static `&str` are written with `fwrite` of the **full** byte range (static via the
  CMS bytes + their known length; non-static via the fat pointer `.data`/`.len`). **No NUL
  termination, no stack buffer, no cap, no truncation, no truncation marker.** Remove the
  entire normative overflow/truncation contract (the 256-cap / `min(len, cap-1)` /
  "truncation whenever `source.len() >= cap`" / per-`%s` buffer rules).
- **§8.4** — keep `max_str_args_per_call = 8` (bridge-side, §26.8 D8, unenforced); replace
  the stack-budget rationale ("bounds per-call stack use at `8 * cap` bytes") with a note
  that, post-`fwrite`, no per-`&str` stack buffer is allocated, so the bound is now purely
  a bridge-side argument-count ceiling. **Do not change the value.**
- **§8.5** — reword the emission description (currently "emits the `printf` call with each
  peeled operand rendered element-by-element through §8.2's per-arg specifier") to the
  uniform-`fwrite` sequence; **retain** the binding rule (read the post-peel flat operands;
  MUST NOT reconstruct a `LogArg` slice / pre-peel carrier — §26.9 uniformity, the
  zero-runtime-cost-on-emit guarantee).

**And the narrow `spec.md` §26 generic-C-binding corrections** (the four stale restatements;
nothing else in §26 changes — see Input Context "IR-side prose to correct"):

- **§26.1** (`:12420`) — the parenthetical "the generic-C target's `printf` with leading
  severity tag" → "the generic-C target's libc-stdio (`fwrite`) emission with leading
  severity tag" (only "`printf`" changes; keep the Pipeline-A flat-operand statement).
- **§26.8 D8** (`:12894`) — reword the trailing rationale "Bounds per-call stack use
  against the per-`%s`-site temporary-buffer allocation" to a bridge-side argument-count
  ceiling (no per-`%s` buffer post-`fwrite`); the D8 trigger + value are unchanged.
- **§26.9** (`:12930`) — "hosted userspace, libc, `printf`" → "libc stdio". Leave the
  general cross-backend Pipeline-A uniformity rule (`:12942-12955`) as-is.
- **§26.10** (`:13005-13013`) — the **generic-C** clause "static-literal direct emission +
  temporary stack-buffer fallback for non-static `&str`" → "byte-faithful `fwrite` emission
  (no NUL-termination needed)". Leave the per-target framing for phase-2 targets.

### Change F: re-bless the corpus, repurpose the truncation fixture, add `diag_nul`, regenerate the ledger

- **Re-bless all** existing `diag_*` goldens via `--bless` (the emission shape changes for
  every fixture). For 9 of them the hand-authored `expected.stdout` is **unchanged** (same
  observable output — e.g. `diag_escapes` still prints `[INFO] {x} 50% done`,
  `diag_newline` still prints the two lines). Review each regenerated `golden.c` against the
  Change-A shapes before committing (never auto-bless `expected.stdout`).
- **Repurpose `diag_str_truncation` → `diag_str_long`.** Under `fwrite` a ≥256-byte
  non-static `&str` prints **in full** (the truncation contract is gone). Rename the
  fixture dir to the now-honest name, set `expected.stdout` to the **full** string (the
  long body + surrounding text + `\n`), keep `covers` = DL-4 (non-static `&str`, now
  full-length — proving no cap/no truncation). (The ledger's DL-4 row records fixture
  names — `coverage_ledger.md:235` — so the rename is reflected on regen.)
- **Add `diag_nul`** — the interior-NUL regression. One self-contained `regseq_main`
  exercising all three paths (each "log a computed value"):
  - **template-literal NUL**: `info!("a\0b={}", n)` (a computed `n`);
  - **static `&str` value NUL**: `info!("s={}", "x\0y")`;
  - **non-static `&str` value NUL**: `info!("d={}", id(&"p\0q"[..]))` with a
    `#[inline(never)] fn id(s: &str) -> &str { s }`.
  `expected.stdout` carries the **literal NUL bytes** (e.g. authored via
  `printf '[INFO] a\0b=<n>\n[INFO] s=x\0y\n[INFO] d=p\0q\n' > expected.stdout`) — **hand-
  authored from the known-correct bytes**, never auto-blessed (the honesty invariant). The
  oracle compares byte-exactly and NUL is valid UTF-8 (Input Context / B5). `covers` =
  DL-3 (the numeric template arg) + DL-4 (static + non-static `&str`); it is a **regression
  guard**, not new coverage (no new matrix row — the existing rows stay green). If a path
  turns out to be bridge-rejected (the task-e probe indicates none are), narrow the fixture
  and flag it.
- **Regenerate** `coverage_ledger.{json,md}` (Mode A) + commit: **no row flips** (DL/TM
  rows stay green; verdict stays `not-pass`); the DL-4 fixture-name list reflects the
  rename; **byte-stable** across two Mode-A runs ([[codegen-ledger-determinism]],
  [[golden-c-emit-process-determinism]]).

### Change G: tests

- **`mod_tests.rs`** — rewrite the `:4985+` emission-shape assertions to the
  uniform-`fwrite` shape: a numeric default → `fwrite("[INFO] x=", 1, 9, stdout); { char
  _n0[24]; int _l0 = snprintf(_n0, sizeof _n0, "%u", (unsigned)(…)); fwrite(_n0, 1, _l0,
  stdout); } fwrite("\n", 1, 1, stdout);`; two `&str` args → two value `fwrite`s (no
  `_logstr`/`[256]`/`%s`); the literal-`fwrite` **byte count equals the decoded length**
  (incl. an embedded `\n` counted as 1 and an interior `\0` rendered `\000` counted as 1);
  the empty template → one merged `fwrite("[INFO] connected\n", 1, 16, stdout)`. Keep the
  parser / specifier-cast / admissible-type tests (`:5132` etc.) unchanged. Remove the
  `c_escape(_, true)` assertion (`:5241`).
- **`generic_c_bodies.rs`** — add compile-**and-run** cases (byte-exact stdout):
  - **template-NUL**: `info!("a\0b={}", 7u32)` → stdout `[INFO] a\0b=7\n` (the NUL present).
  - **static-`&str`-NUL**: `info!("s={}", "x\0y")` → `[INFO] s=x\0y\n`.
  - **non-static-`&str`-NUL**: a runtime `&str` with an interior NUL → printed in full.
  - **long non-static `&str`** (≥256): printed in full (no truncation) — the `diag_str_long`
    analogue.
  Keep the existing diag behavioral cases (they assert stdout/exit, unaffected by the
  emission-shape change).

## Non-Goals

- **No bridge / model change**, and **no `spec.md` change beyond the four narrow §26
  generic-C-binding-prose corrections** (§26.1 "printf" example, §26.8 D8 rationale, §26.9
  "printf", §26.10 generic-C `&str` clause) the redesign falsifies. The IR producer-surface proper — the
  §26.7 grammar, the §26.6 peel/translation contract, the D-rule triggers/values, and the
  Pipeline-A prohibition — is unchanged (the IR always permitted interior-NUL `&str`; the
  substantive fix is the C-target binding). DL-5/D8 stays bridge-side + unenforced.
- **No prelude change** (`fwrite`/`snprintf` in `<stdio.h>`; `<string.h>` retained for
  other `memcpy` callers).
- **No Doc-1 matrix `status` edit** — green-ness lives in the regenerated ledger.
- **No change to the specifier/cast mapping, the template parser, the static-vs-non-static
  discriminator, or the numeric-`Ty` recovery** — all reused verbatim; the only behavioral
  change is the **emission shape** (`printf`/`%s`/stack-buffer → uniform `fwrite` +
  `snprintf`).
- **No new `EmitError` variant** — the existing defensive `Unsupported(&'static str)` arms
  (unclosed/stray brace, out-of-subset modifier, non-admissible `Ty`, hex-on-`&str`,
  `None` target, arity mismatch, non-literal `args[0]`) are reused.
- **No truncation behaviour** — `&str` of any length prints in full (the §8.3 cap/truncation
  contract is **removed**, not re-implemented).
- **`expected.stdout` is never auto-blessed** (Change E/F honesty invariant); the
  `diag_nul` / `diag_str_long` expectations are hand-authored from the known-correct bytes.

## Test Plan

### B1: build + workspace green
- `cargo build -p regseq-backends -p codegen-coverage-runner` clean.
- `RUST_TEST_THREADS=1 cargo test -p regseq-backends -p codegen-coverage-runner --
  --test-threads=1` green (every prior task's test + the rewritten/added task tests).

### B2: backend emission — compile-**and-run** (in-crate `generic_c_bodies.rs`)
- All task-e behavioral cases (levels, numeric widths incl. a `%llu`-distinguishing value,
  bool, hex incl. a 64-bit `%08llx` + signed hex at every width, static & non-static `&str`,
  empty template, escapes, embedded newline) **still pass byte-exactly** under the new
  emission (same observable stdout).
- **Interior-NUL (the regression):** template-NUL, static-`&str`-NUL, non-static-`&str`-NUL
  each print the **full** byte sequence including the NUL — byte-exact stdout (the task-e
  printf path would have truncated at the NUL); exit 0.
- **Long non-static `&str`** (≥256) prints **in full** — no truncation, no marker; exit 0.

### B3: structural (`mod_tests.rs`)
- The uniform-`fwrite` shape: tag merged into the leading literal `fwrite`; `\n` merged
  into the trailing literal `fwrite`; one numeric `snprintf`+`fwrite` block per numeric
  placeholder with buffer `[24]` and `fwrite(_n<k>, 1, _l<k>, stdout)`; one value `fwrite`
  per `&str` (static = C-literal-from-CMS + explicit count; non-static =
  `fwrite((..).data, 1, (..).len, stdout)`, **no** `_logstr`/`[256]`/`memcpy`/`%s`).
- The literal `fwrite` **byte count = decoded length** (an embedded `\n` → 1; an interior
  `\0` rendered `\000` → 1) — not the escaped-literal length.
- The reused mapping is intact (`diag_specifier_and_cast_for` arms; the admissible-type
  whitelist rejecting `Char`/`Isize`/`Usize`; hex-on-`&str` → `Unsupported`); the parser
  arms; a `None` target / arity mismatch → `Unsupported`.

### B4: spec delta sanity
- `generic-c-backend.md` §1 scope line no longer names `printf` (→ "libc stdio"); §7.1 no longer says "emit the §8 `printf`"; §8.1/§8.2/§8.3/§8.4/§8.5 read coherently after the edit; §8.3 no
  longer references a cap / truncation / stack buffer / NUL termination; §8.4 keeps the
  value 8 with the updated rationale; §8.5 no longer says "printf" / "all operands through a
  specifier" but keeps the no-`LogArg`-reconstruction rule; no other §8 prose perturbed.
- `spec.md` §26.1 / §26.8 D8 / §26.9 / §26.10 no longer restate the generic-C stack-buffer /
  `printf` / NUL-termination mechanism; the §26.7 grammar, §26.6 peel contract, the D-rule
  triggers/values, and the §26.9 Pipeline-A prohibition are **untouched**; no other §26
  prose perturbed.

### B5: end-to-end — the re-blessed corpus + `diag_nul` (Change F)
- Mode A over the corpus: **every fixture PASSes** all six stages (acquire → emit →
  snapshot vs the **re-blessed** `golden.c` → compile → run → byte-exact stdout oracle),
  including `diag_nul` (stdout with literal NUL bytes matches `expected.stdout` byte-exactly)
  and `diag_str_long` (full ≥256-byte output); **no fixture FAILs**.
- The ledger keeps DL-1/2/3/4/6/7 (+ co-tagged TM/body rows) **green**, verdict
  **`not-pass`**, and is **byte-stable** across two Mode-A runs; the DL-4 row reflects the
  `diag_str_long` rename.

### B6: fmt + clippy + diff scope
- `cargo fmt --all -- --check` clean; `cargo clippy -p regseq-backends -p
  codegen-coverage-runner --all-targets -- -D warnings` clean (scoped per
  [[clippy-all-targets-test-drift]]).
- Diff confined to: `crates/regseq-backends/src/generic_c/body_emit.rs` (+ `mod_tests.rs`);
  `docs/discussions/regseq-model/generic-c-backend.md` (§8 + the two one-line §1 / §7.1
  cross-ref clauses); `docs/discussions/regseq-model/spec.md` (the four §26.1/§26.8/§26.9/§26.10
  generic-C-binding clauses only); the `crates/codegen-coverage-runner/fixtures/diag_*/` corpus (re-blessed
  goldens; the `diag_str_truncation`→`diag_str_long` rename; `+ diag_nul`);
  `crates/codegen-coverage-runner/tests/generic_c_bodies.rs`; the regenerated
  `.harness/generic-c/coverage_ledger.{json,md}`; `tasks.json`. **No** bridge / model /
  `prelude.rs` / `types.rs` / `layout_emit.rs` / `enum_emit.rs` / `naming.rs` / runner
  `src/*.rs` change; **no `spec.md` edit beyond the four named §26 generic-C clauses**.

## Review

### Files to send to Codex
This spec; `docs/discussions/regseq-model/generic-c-backend.md` §1 (`:26-33`) + §7.1
(`:359-365`) + §8 (**before** the edit — review the delta plan); `docs/discussions/regseq-model/spec.md` §26.6/§26.7 (unchanged-
confirmation) **and §26.1 `:12405-12426` / §26.8 `:12873-12894` / §26.9 `:12922-12955` /
§26.10 `:12957-13016`** (the narrow generic-C-binding corrections);
`docs/discussions/generic-c-backend/phase-1/03-verification-flow.md`;
`crates/codegen-coverage-runner/fixtures/README.md`;
`crates/regseq-backends/src/generic_c/body_emit.rs` (`emit_diag_log` `:1961-2074`,
`diag_specifier_and_cast_for` `:3709`, `parse_template` `:3814`, `c_escape` `:3934`, the
reused readers `:2082-2166`); `crates/regseq-backends/src/generic_c/mod_tests.rs:4985-5420`
(the §8 block; the `emit_diag_call` emission-shape tests `~:5357-5420`; the `c_escape` unit
test `~:5239-5245`);
`crates/codegen-coverage-runner/src/{oracle.rs, toolchain.rs:88-120, pipeline.rs}` (the
byte-exact oracle + `from_utf8_lossy` capture);
`crates/codegen-coverage-runner/tests/generic_c_bodies.rs`; the existing
`crates/codegen-coverage-runner/fixtures/diag_{escapes,newline,str_nonstatic,str_truncation}/`
(golden + expected.stdout, to ground the re-bless).

### Reviewer focus
- **Faithfulness (the whole point).** Does the uniform `fwrite` reproduce **every** byte,
  incl. interior NUL, on **all three** paths (template literal, static `&str` value,
  non-static `&str` value) and the numeric/literal interleave? Is the leading tag + trailing
  `\n` placement correct and deterministic (merged into adjacent literal runs; standalone
  when the template starts/ends with a placeholder or is empty)?
- **`snprintf`/`fwrite` soundness (NEW UB surface).** Is the buffer provably large enough
  that `snprintf` **never truncates** so the returned `_l<k>` is exact and `fwrite` never
  over-reads? Verify the 20-byte max claim (`i64::MIN`/`u64::MAX` decimal; hex ≤ 18;
  `Isize`/`Usize` excluded by the whitelist) against the `[24]` buffer. Is `_l<k>`
  guaranteed non-negative (no encoding error for integer conversions)?
- **Literal byte count.** Is the `fwrite` count the **decoded** byte length (embedded `\n`
  = 1, interior `\0` = 1) and **not** the escaped C-literal length? (An off-by-escaping bug
  would over/under-write.)
- **`&str` `fwrite`.** Non-static = `fwrite((op).data, 1, (op).len, stdout)` — exactly
  `.len` bytes (no over-read; the fat pointer is a bridge certificate); static = the CMS
  bytes as a C literal with the explicit decoded count; **no** residual buffer / cap /
  truncation / NUL-append; the static-vs-non-static discriminator unchanged.
- **Ordering.** `fwrite` and `snprintf`-buffer-`fwrite` calls interleave on one `stdout`
  stream in program order — bytes land in call order (no reordering); the fixtures' `main`
  flushes at exit.
- **Reuse discipline.** `diag_specifier_and_cast_for` / `parse_template` / `parse_placeholder`
  / `require_default_modifier` / `read_*` / `is_str_ref_place` / `diag_numeric_ty`
  **unchanged**; the only behavioral change is the emission shape.
- **`c_escape` orphan removal.** The `double_percent` param is genuinely dead (no `true`
  caller remains); removal updates only the two call sites + the one test; no other caller.
- **Spec delta scope (E).** §8.1/§8.2/§8.3/§8.4 only; the truncation contract is **removed**
  (not just relaxed); §8.4 value unchanged; no `spec.md`/§26.7 edit; no other §.
- **Fixture honesty (F).** Re-blessed goldens reviewed; `expected.stdout` never
  auto-blessed; `diag_nul`/`diag_str_long` expectations hand-authored from known-correct
  bytes (incl. literal NUL); "log a computed value"; honest `covers`; ledger byte-stable, no
  row flips, verdict `not-pass`, no matrix edit.
- **No regression.** All task-e NUL-free behavioral cases still pass byte-exactly.

### Adversarial prompt template
```
<task>
Review .harness/tasks/regseq-backends/16-generic-c-diag-log-fwrite-faithfulness.md — the
Phase-1 follow-up to task e (regseq-backends-15): the generic-C diag-log emission is
redesigned from a single NUL-terminated printf(fmt,args) to a UNIFORM fwrite-based emission
to fix a real interior-NUL &str silent truncation (the 5th soundness-pass catch). Every byte
that reaches stdout is written via fwrite(ptr,1,n,stdout) with an explicit byte count:
literal runs (tag + template text + trailing \n) and &str VALUES via fwrite directly; numeric
conversions via snprintf into a fixed [24] stack buffer then fwrite of the returned length.
This DELETES the §8.3 256-cap/min(len,cap-1)/no-marker truncation contract (no buffer, no cap,
no truncation — a &str of any length prints in full) and is a deliberate §1/§7.1/§8.1/§8.2/§8.3
(+§8.4 rationale/§8.5) canonical-spec delta in generic-c-backend.md. The specifier+cast mapping
(diag_specifier_and_cast_for), the template parser, the static-vs-non-static discriminator,
and the numeric-Ty recovery are REUSED UNCHANGED; the c_escape double_percent param is dropped
(orphaned — literals are no longer formatted). Re-bless all diag goldens; repurpose
diag_str_truncation→diag_str_long (≥256 prints in full); add diag_nul (template + static +
non-static interior NUL; expected.stdout carries literal NUL bytes; the byte-exact oracle +
from_utf8_lossy capture preserve NUL as valid UTF-8). The §8 delta also requires four narrow
spec.md §26 corrections of the now-stale generic-C-binding restatements (§26.1 "printf"
example, §26.8 D8 rationale, §26.9 "printf", §26.10 generic-C &str clause); OUT OF SCOPE: spec.md §26.6/§26.7 grammar +
the Pipeline-A prohibition (untouched), bridge, model, prelude, the matrix status column,
DL-5/D8 value (bridge-side, unenforced). This is a SPEC review: judge correctness,
completeness, internal consistency, sound scoping, faithful grounding vs HEAD 6789161.
SPECIFICALLY verify: (1) the snprintf buffer is provably large enough that it never truncates
so the returned length is exact and fwrite never over-reads (the 20-byte max claim:
i64::MIN/u64::MAX decimal; hex ≤18; Isize/Usize excluded by the whitelist; [24] buffer);
(2) the literal fwrite COUNT is the decoded byte length (embedded \n=1, interior \0 rendered
\000=1), NOT the escaped-literal length; (3) the non-static &str fwrite reads exactly .len
bytes (no over-read) and the static path counts decoded bytes; (4) interior NUL is faithful on
ALL THREE paths; (5) the tag/\n merge is deterministic incl. the placeholder-first / empty
template edges; (6) ordering across interleaved fwrite/snprintf-fwrite is well-defined on one
stdout stream; (7) the spec delta removes (not merely relaxes) the truncation contract and
touches §8 + the two one-line §1/§7.1 cross-refs only; (8) the c_escape param removal is sound (no remaining true caller);
(9) fixture honesty (no auto-bless; NUL expected.stdout hand-authored; byte-stable ledger; no
row flips; verdict not-pass); (10) scope creep.
Identify:
- Emission rewrite (A): pending-literal accumulator; flush-before-placeholder; append-\n-then-
  flush; goto target no-dest-assign; arity parity; Pipeline-A.
- &str fwrite (B): static = CMS-bytes C-literal + decoded count; non-static =
  fwrite((op).data,1,(op).len,stdout); no buffer/cap/truncation/NUL; discriminator unchanged.
- Numeric snprintf+fwrite (C): reuse diag_specifier_and_cast_for verbatim; [24] buffer
  no-truncation proof; _l<k> exact & non-negative; per-call block scope.
- Literal fwrite + c_escape (D): decoded byte count; no %-doubling; param removal orphan-safe.
- Spec delta (E): generic-c-backend.md §1 + §7.1 cross-refs off "printf"; §8.1 uniform-fwrite; §8.2 Ref(Str) row; §8.3
  retitle+remove truncation; §8.4 value kept + rationale; §8.5 off "printf"/all-specifier
  but keep no-LogArg rule. spec.md §26.1 "printf" example + §26.8 D8 rationale + §26.9
  "printf" + §26.10 generic-C &str clause only; §26.6/§26.7 + Pipeline-A prohibition untouched.
- Fixtures+ledger (F): re-bless reviewed; diag_str_long full output; diag_nul three NUL paths +
  literal-NUL expected.stdout (oracle byte-exact, NUL valid UTF-8); honest covers; byte-stable;
  no row flips; verdict not-pass; no matrix edit.
- Tests (G): mod_tests emission-shape rewrite + decoded-count assertions; generic_c_bodies NUL +
  long cases; task-e NUL-free cases still pass; c_escape test update.
- Scope: no bridge/model/prelude/runner-src/naming; no spec.md edit beyond the 4 named §26
  generic-C clauses (§26.1/§26.8/§26.9/§26.10); §26.6/§26.7 + Pipeline-A prohibition
  untouched; diff confined.
Read: docs/discussions/regseq-model/generic-c-backend.md §1 + §7.1 + §8; spec.md §26.1 + §26.6-26.10;
phase-1/03-verification-flow.md; codegen-coverage-runner/fixtures/README.md;
regseq-backends/src/generic_c/body_emit.rs (emit_diag_log :1961-2074, diag_specifier_and_cast_for
:3709, parse_template :3814, c_escape :3934, readers :2082-2166) + mod_tests.rs:4985-5420;
codegen-coverage-runner/src/{oracle.rs,toolchain.rs:88-120,pipeline.rs} + tests/generic_c_bodies.rs;
the existing fixtures/diag_{escapes,newline,str_nonstatic,str_truncation}/.
</task>

<grounding_rules>
- Cite file:line for every claim. Mark hypotheses as hypotheses.
- Don't speculate about code/spec you didn't read.
</grounding_rules>

<structured_output_contract>
VERDICT: APPROVED | APPROVED_WITH_MINOR_FIXES | NOT_READY
EMISSION-REWRITE-A (pending-literal accumulator; flush-before-placeholder; append-\n-flush; goto no-dest-assign; arity parity; Pipeline-A): PASS / FAIL with citation
STR-FWRITE-B (static = CMS C-literal + decoded count; non-static = fwrite((op).data,1,(op).len,stdout); no buffer/cap/truncation/NUL; discriminator unchanged): PASS / FAIL with citation
NUMERIC-SNPRINTF-C (reuse diag_specifier_and_cast_for verbatim; [24] buffer never truncates → _l exact & in-bounds; the 20-byte max proof; per-call block scope): PASS / FAIL with citation
LITERAL-FWRITE-AND-CESCAPE-D (decoded byte count not escaped length; no %-doubling; double_percent param removal orphan-safe): PASS / FAIL with citation
SPEC-DELTA-E (gc §1 + §7.1 cross-refs off "printf"; §8.1 uniform-fwrite; §8.2 Ref(Str) row; §8.3 retitle + truncation contract REMOVED; §8.4 value kept + rationale; §8.5 off printf/all-specifier but keep no-LogArg; spec.md §26.1 printf example + §26.8 D8 rationale + §26.9 printf + §26.10 generic-C &str clause only, §26.6/§26.7 + Pipeline-A prohibition untouched): PASS / FAIL with citation
FIXTURES-AND-LEDGER-F (re-bless reviewed; diag_str_long full output; diag_nul 3 NUL paths + literal-NUL expected.stdout byte-exact via NUL-valid-UTF-8 oracle; honest covers; byte-stable; no row flips; verdict not-pass; no matrix edit): PASS / FAIL with citation
TESTS-G (mod_tests emission rewrite + decoded-count asserts; generic_c_bodies NUL + long cases; task-e NUL-free cases still pass byte-exactly; c_escape test update): PASS / FAIL with citation
NO-SILENT-MISCOMPUTE (snprintf over-read; literal count = escaped-not-decoded; non-static &str over-read; interior NUL still truncated on any path; ordering hazard — all closed): PASS / FAIL with citation
SCOPE-DISCIPLINE (no bridge/model/prelude/runner-src/naming; no spec.md edit beyond the 4 named §26 generic-C clauses §26.1/§26.8/§26.9/§26.10, §26.6/§26.7 + Pipeline-A prohibition untouched; diff confined; mapping/parser/discriminator reused unchanged): PASS / FAIL with citation
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
  codegen-coverage-runner --all-targets -- -D warnings` clean (scoped).
- `RUST_TEST_THREADS=1 cargo test -p regseq-backends -p codegen-coverage-runner --
  --test-threads=1` green (prior tasks + the rewritten/added tests).
- The B2 in-crate cases (all task-e NUL-free cases **still** byte-exact; the **interior-NUL**
  template/static/non-static cases print the full bytes incl. NUL; the **long** ≥256 `&str`
  prints in full) compile **and** run with the asserted stdout + exit 0.
- The B5 re-blessed corpus + `diag_nul` + `diag_str_long` PASS all six stages; the ledger
  keeps DL-1/2/3/4/6/7 (+ co-tagged TM/body rows) **green**, verdict **`not-pass`**, **no
  fixture FAILs**, **byte-stable** across two runs.
- `--bless` writes **only** `golden.c` (never `expected.stdout`); the interior-NUL
  `expected.stdout` is hand-authored.
- The §8 spec delta is internally coherent (incl. §8.5) and the §1 + §7.1 cross-refs are reworded;
  the §8.3 truncation contract is
  **removed**; the four narrow `spec.md` §26.1/§26.8/§26.9/§26.10 generic-C-binding
  corrections are coherent; **no `spec.md` edit beyond those four clauses** (§26.6/§26.7 and the
  Pipeline-A prohibition untouched).
- `git diff --stat` confined to the Quality-Gate set (B6); **no** residual non-static-`&str`
  `_logstr[256]`/`memcpy`/`%s` or `printf(fmt, args)` in the diag path.
- Codex Task Review: APPROVED or APPROVED_WITH_MINOR_FIXES.
- **Mandatory post-close Codex soundness pass** ([[assessment-soundness-pass]]) — this task
  is itself a soundness fix and re-opens a UB surface (`snprintf` return-length vs `fwrite`
  over-read; the literal decoded-count; the non-static `&str` `.len` over-read). The pass is
  the 6th after d1/d2/d2.1/d2.2/e; specifically re-verify the `snprintf` buffer bound and
  that interior NUL is faithful on all three paths (the green fixtures must not re-hide a
  hole).

## Acceptance Criteria

1. **Uniform-`fwrite` emission** (Change A, `body_emit.rs`): `emit_diag_log` lowers each
   call to a deterministic statement sequence — a leading literal `fwrite` (the tag merged
   into the first literal run), interleaved numeric `snprintf`+`fwrite` blocks and `&str`
   value `fwrite`s in template order, a trailing literal `fwrite` ending in `\n` (merged
   into the last literal run; standalone when the template ends with a placeholder/is
   empty), then `goto bb_<target>` with no destination assignment. The arity check and
   Pipeline-A (no `LogArg`) are retained; a `None` target / arity mismatch → `Unsupported`.
2. **`&str` byte-faithful `fwrite`** (Change B): a **static** `&str` value emits
   `fwrite("<CMS bytes>", 1, <decoded len>, stdout)`; a **non-static** `&str` value emits
   `fwrite((<expr>).data, 1, (<expr>).len, stdout)` — **no** stack buffer, cap, truncation,
   or NUL append; the task-e `_logstr<k>[256]`/`memcpy`/`min(len,255)` prelude is deleted;
   the static-vs-non-static discriminator (`read_static_str_bytes`/`is_str_ref_place`) is
   reused unchanged; the full byte range (interior NUL included) is reproduced.
3. **Numeric `snprintf`+`fwrite`** (Change C): each numeric placeholder renders via
   `snprintf` into a fixed `[24]` stack buffer using `diag_specifier_and_cast_for`'s
   **unchanged** `(spec, cast)` (the `ll`-for-64-bit-hex, the two-step source-width
   signed-hex cast, the admissible-type whitelist), then `fwrite(_n<k>, 1, _l<k>, stdout)`.
   The buffer is **provably larger** than the maximum admissible rendering (20 bytes —
   `i64::MIN`/`u64::MAX` decimal; hex ≤ 18; `Isize`/`Usize` non-admissible), so `snprintf`
   never truncates → `_l<k>` is exact, non-negative, and `fwrite` never over-reads; no
   clamp.
4. **Literal `fwrite` + `c_escape`** (Change D): each literal run (incl. the tag and `\n`)
   emits `fwrite("<c_escape(bytes)>", 1, <decoded byte len>, stdout)` with the count equal
   to the **decoded** byte length (embedded `\n` = 1, interior `\0` rendered `\000` = 1),
   not the escaped-literal length, and **no `%`-doubling**; the now-orphaned `c_escape`
   `double_percent` parameter is removed (both call sites + the unit test updated).
5. **Spec delta** (Change E): in `generic-c-backend.md` — §1's scope line and §7.1's "emit
   the §8 `printf`" cross-ref reworded off `printf` (→ libc stdio / the stdio emission
   sequence); §8.1 single-emitter → uniform-
   `fwrite`; §8.2 `Ty::Ref(Str)` row → byte-faithful `fwrite`; §8.3 retitled and rewritten
   with the truncation/cap/buffer/NUL contract **removed**; §8.4 keeps the value 8 with the
   updated rationale; §8.5 reworded off "`printf`"/"all-operands-through-specifier" while
   keeping the no-`LogArg`-reconstruction rule. In `spec.md` — the four narrow
   generic-C-binding corrections (§26.1 "printf" example, §26.8 D8 rationale, §26.9 "printf",
   §26.10 generic-C `&str` clause) only; **no** §26.6/§26.7/Pipeline-A-prohibition/other-section edit.
6. **Re-blessed corpus + `diag_nul` + `diag_str_long`** (Change F): all diag goldens are
   re-blessed (reviewed; `expected.stdout` unchanged for the 9 NUL-free fixtures);
   `diag_str_truncation` is repurposed/renamed to `diag_str_long` printing a ≥256-byte
   `&str` **in full**; `diag_nul` exercises all three interior-NUL paths with a hand-authored
   literal-NUL `expected.stdout`; the corpus PASSes all six stages; the ledger is
   regenerated + committed with DL-1/2/3/4/6/7 (+ co-tagged rows) **green**, verdict
   **`not-pass`**, **byte-stable**, the DL-4 row reflecting the rename; no Doc-1 matrix edit;
   `--bless` never writes `expected.stdout`.
7. **Tests** (Change G): `mod_tests.rs` emission-shape assertions rewritten to the
   uniform-`fwrite` shape (incl. the decoded-count assertion); the mapping/parser/whitelist
   tests retained; `generic_c_bodies.rs` adds interior-NUL (template/static/non-static) +
   long-`&str` compile-and-run cases; all task-e NUL-free behavioral cases still pass
   byte-exactly; the `c_escape(_, true)` test removed.
8. **Soundness + scope**: the interior-NUL silent truncation is fixed on all three paths;
   no new UB (`snprintf` buffer bound proven, no `fwrite` over-read, decoded literal count,
   `.len`-exact non-static read, well-defined stdout ordering); **no scope creep** — no
   bridge/model/prelude/runner-src/`naming.rs` change, and **no `spec.md` edit beyond the
   four named §26 generic-C-binding clauses**; the diff is confined to the Quality-Gate
   file set; no residual `printf(fmt,args)`/`%s`/`_logstr[256]` in the diag path.
