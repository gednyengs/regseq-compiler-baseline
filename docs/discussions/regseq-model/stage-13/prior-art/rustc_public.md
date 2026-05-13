# rustc_public prior-art: pretty-printing

Primary-source pass on rustc_public's MIR pretty-
printer, framed by the stage-13 D-tag set. Per README §2
this is a fresh pretty-printing-lens read.

All paths are relative to the pinned toolchain at
`~/.rustup/toolchains/nightly-2026-02-01-aarch64-apple-darwin/lib/rustlib/rustc-src/rust/compiler/`.

## Sources

- `rustc_public/src/mir/pretty.rs` (~466 lines) --
  the stable-IR MIR pretty-printer; the structurally
  closest analog to RegseqModel's situation (owned
  IR, no `'tcx`, designed for external consumption).
- `rustc_public/src/rustc_internal/pretty.rs` (~21
  lines) -- the rustc-internal-bridge pretty helper
  (tiny; one entry point).

## Per-D-tag relevance

### D-13-1 (master rendering style + method shape)

- **Style: Rust-source-style.** Same convergence as
  rustc-internal -- `&{mut }T`, `[T; N]`, etc. No
  divergence on textual choice.
- **Method shape: free `pretty_X` fns over
  `&mut W: io::Write`.** `pretty_X` helpers are
  standalone module-level functions threading a
  writer parameter, NOT Debug-impls and NOT methods
  on a context-bearing struct. Entries:
  `function_body` (`mir/pretty.rs:38`); `pretty_statement`
  (`:90`); `pretty_terminator` / `pretty_terminator_head`
  (`:128` / `:171`); `pretty_successor_labels`
  (`:204`); `pretty_assert_message` (`:229`);
  `pretty_operand` (`:326`); `pretty_rvalue` (`:347`);
  `pretty_aggregate` (`:397`).
- **Closest match for RegseqModel's
  `PrettyPrinter` shape.** Free fns on `&mut W` map
  cleanly to methods on a context-bearing
  `PrettyPrinter` returning `String`: in both
  shapes the renderer threads a "where do I write"
  parameter (the writer for rustc_public, the
  `String` builder + `&self.model` for RegseqModel)
  while the per-variant dispatch is module-level.

### D-13-2 (body envelope + indentation)

- **Same envelope as rustc-internal:**
  `fn name(_1: T, ...) -> T { let mut _N: T; ...; bbN: { stmts; terminator -> [...]; } }`.
  `function_body` (`:38`) writes the function header
  + locals + reachable basic blocks.
- **Indentation.** `INDENT` for block body = 4 spaces
  (constant `INDENT` declared inline per fn around
  `:91` for statements, `:173` for terminators). For
  statement body inside a block: 8 spaces. Same
  rustc-converged values.

### D-13-3 (span-rendering mode)

- **NO span rendering at all.** rustc_public's
  `pretty.rs` omits all span-related output --
  neither inline nor out-of-line nor toggle-gated.
  Counter-evidence: a stable IR pretty-printer can
  ship with zero span content and still be useful
  for snapshot tests / diagnostics.
- **Implication for D-13-3.** Stage 13 must decide
  whether RegseqModel's snapshot tests want spans at
  all. Three viable positions: rustc-style toggle
  (default-off), rustc_public-style omission
  (always-off), or always-on. Snapshot stability
  considerations may push toward rustc_public's
  always-off default with optional toggle.

### D-13-4 (Item-level rendering shape)

- **rustc_public renders Body content, not Items.**
  Item-level rendering for `MonoItem { Fn, Static,
  GlobalAsm }` lives elsewhere in rustc_public (not
  in `mir/pretty.rs`). No primary evidence for
  D-13-4 from this file.
- **Implication.** Like rustc-internal,
  rustc_public doesn't directly evidence the
  Item-level rendering shape RegseqModel needs;
  the D-12-K bundle's renderers
  (`render_item`, ItemKind variant rendering,
  Linkage / InlineHint / Function-flag /
  link_section / opaque_handle_eligible decorators)
  must synthesize from analogous patterns.

### D-13-5 (`render_ty` per-arm specifications)

- rustc_public delegates type rendering to the
  thread-local context (`with(|ctx| ...)`); types
  are rendered through the public Ty's `Debug` impl
  which calls back into the rustc-internal type
  printer via the bridge.
- **Implication for D-13-5.** rustc_public doesn't
  directly evidence per-arm Ty rendering -- it
  defers to rustc-internal. Stage 13's `render_ty`
  per-arm specs inherit primarily from rustc-internal
  ty pretty-printer + Charon's owned-IR analog.

### D-13-6 (body-level renderer per-arm specifications)

- **`pretty_assert_message` (`:229`) is a concrete
  enumeration model** for `render_assert_kind` /
  D-9-K's 8-variant `AssertKind`. Per-variant arms
  emit fixed-shape templates with `{}` placeholders
  for operand parameters: BoundsCheck /
  Overflow(Add/Sub/Mul/Div/Rem/Shr/Shl) / Negate /
  DivZero / RemZero / Misalign / NullDeref /
  InvalidEnum / Resumed*. This is the direct model
  for RegseqModel's `render_assert_kind` arms.
- **`pretty_aggregate` (`:397`) is a manual variant
  dispatch over `AggregateKind`** with per-variant
  textual templates. RegseqModel's `render_rvalue`
  Aggregate-arm follows the same pattern (D-7-1.7
  7-variant `AggregateKind`).
- **`pretty_terminator_head` (`:171`) +
  `pretty_successor_labels` (`:204`) split** the
  per-variant text from the post-arrow successor
  list. Same split available to RegseqModel's
  `render_terminator` -- carries forward as a sub-
  decision under D-13-6.5.
- **Place rendering punts to Debug.** `Place` uses
  `{place:?}` (the Debug impl on the public Place
  type, which calls back via thread-local context).
  `pretty_X` helpers don't render Place themselves.
  RegseqModel can't use this pattern (Place projection
  resolution needs `&Body` for type inference per
  §19); methods-on-PrettyPrinter is the carry-
  forward.
- **No place-projection wrapping in `pretty.rs`.**
  rustc_public's `Place` Debug impl (in
  `rustc_public/src/mir/body.rs:680`) handles
  projection rendering directly, not as a
  pre/post-wrap split. Pattern: simpler post-only
  chain (rustc-internal-style pre/post-wrap may not
  be necessary).

### D-13-7 (worked-example specimens)

- No worked-example specimens within
  `rustc_public/src/mir/pretty.rs`. Tests live in
  `tests/ui/stable-mir-print/` and similar fixture
  directories outside this file.
- **Implication for D-13-7.** Same as rustc-internal:
  worked-examples belong in spec + insta snapshot
  fixtures, not in the renderer source.

### D-13-8 (sigil tables / ownership tables)

- **No documented sigil convention** in
  `rustc_public/src/mir/pretty.rs`. Local renders
  `_N` via Place's Debug impl + body.rs printing
  conventions.
- **No per-method ownership table analog.**
  rustc_public's pretty-printer is a single module
  with module-level free fns; no
  multi-stage-ownership tracking.

## Key takeaway for stage 13

rustc_public is the LOAD-BEARING precedent for
RegseqModel's structural shape: free helpers
(`pretty_X` over `&mut W: io::Write`) map cleanly to
methods on a context-bearing `PrettyPrinter`
returning `String`. Stage 13's D-13-1 candidate
(Rust-source-style + `PrettyPrinter` methods)
synthesizes rustc-internal's textual choices with
rustc_public's structural pattern.

Concrete adoption-targets:
- `pretty_assert_message` enumeration pattern -> direct
  model for `render_assert_kind`.
- `pretty_aggregate` per-variant textual templates
  -> direct model for Aggregate-arm rendering in
  `render_rvalue`.
- `pretty_terminator_head` + `pretty_successor_labels`
  split -> direct model for `render_terminator` /
  `render_terminator_kind` separation.
- Always-off span rendering -> viable D-13-3
  candidate (counter-evidence to rustc-internal's
  toggle default).

## Cross-stage drift / counter-evidence

- **rustc_public ships zero span rendering**
  (counter-evidence to rustc-internal's toggle
  pattern). Both are viable; stage 13 picks based on
  RegseqModel's snapshot-test stability needs.
- **Place rendering punts to Debug** in
  rustc_public; RegseqModel's `render_place` needs
  `&Body` for projection type resolution (D-5-1's
  closed 6-variant `PlaceElem` enum + per-arm
  `place_ty` walk). The free-fn-with-Debug-Place
  pattern doesn't carry; the methods-on-
  PrettyPrinter shape RegseqModel already adopted is
  the right call.
