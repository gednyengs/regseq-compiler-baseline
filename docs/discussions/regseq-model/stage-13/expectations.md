# Expectations on stage 13 (incoming)

Stage 13 covers type-system pretty-printing: filling
`pretty.rs::render_ty` and any related rendering concerns
required to pretty-print the `Ty` enum once all its content
sources have been filled (FnSig from stage 11; Item / DefPath
from stage 12). Stage 13's rendering-style decision for `Ty`
also serves as the input for stage 14's `render_const`
(deferred from stage 4 per D-4-K because `render_const`'s
dependency chain is longer than `render_ty`'s — see
`../stage-14/expectations.md` "From stage 4").

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

## From stage 3

- [ ] **Fill `pretty.rs::render_ty` stub.** The stub at
  `crates/regseq-model/src/pretty.rs` panics with
  `unimplemented!("stage 3 fills render_ty")` (the original
  scaffold message; if updated as part of this stage's work,
  rename to "stage 13 fills render_ty"). Stage 3 originally
  reserved this slot as an open decision (informally "D14 --
  renderer style") but deferred it: the renderer's input depends
  on `Ty` content references that only become fully populated
  by stage 11 (`FnSig`) and stage 12 (`Item` / DefPath). Locking
  the renderer style at stage 3 would have produced a spec that
  changes shape every time later stages fill in content.

  *Stage-3 lean (preserved as input, not binding).* Rust-source-
  style notation: `&'r T`, `*const T`, `[T; N]`, `dyn Trait`,
  `for<'a> fn(...)`, etc. Charon precedent. See
  `../stage-3/discussions.md` (the historical D14 entry,
  preserved as deferred-decision context). The lean was
  motivated by stage 3's wish to mirror what humans already read
  in rustc's `--emit=mir` output and by the precedent set by
  Charon. Stage 13 is free to validate this lean against the
  fully-populated input domain, or to choose differently
  (constructor-style, C-source-style, or other) -- the call
  belongs to stage 13.

  *Stage-13 deliverables.*

  1. Make the renderer-style call. Pin the chosen style as a
     stage-13 decision (mint a fresh `D-13-X` tag).
  2. Specify the rendering for each variant of the closed
     19-variant `Ty` enum (see `../stage-3/spec-snapshot.md`
     Section 1.1 for the variant list). Cover ADT-shaped
     variants (which call into `render_def_path`, owned by
     stage 12), `Ty::FnPtr` (which expands its `FnSig`, owned
     by stage 11), `Ty::Dynamic` (auto-traits + vtable
     reference), `Ty::Tuple` (synthetic field names), and the
     unsized markers (`Str`, `Slice`, `Never`).
  3. Implement the renderer. Coordinate with stage 11's
     `render_local`, stage 12's `render_def_path`, and stage
     14's `render_instance` -- the four renderers should
     produce coherent output (consistent quoting, sigils,
     verbosity).
  4. Update insta snapshots accordingly.

  *Cross-refs.*
  - `../stage-3/spec-snapshot.md` Section 1.1 -- the variant
    table the renderer must handle.
  - `../stage-3/discussions.md` -- historical D14 entry
    (deferred), preserves the Rust-source-style lean and the
    options that were considered (constructor style, C-source
    style).
  - `../stage-3/prior-art/charon.md` -- Theme G (Charon's
    renderer style, the prior-art evidence behind the
    Rust-source-style lean).
  - `../stage-1/spec-snapshot.md` -- pretty-printer scaffold
    interface and the existing `render_ty` stub.
  - `../stage-11/expectations.md` -- coordination on
    `render_local` style and `FnSig` content.
  - `../stage-12/expectations.md` -- coordination on
    `render_def_path` style and `Item` content.
  - `../stage-14/expectations.md` -- coordination on
    `render_instance` style.

## Bridge concerns

Stage 13 has no bridge-side work: pretty-printing is consumer-
side (codegen, snapshot tests, diagnostics) and reads the
already-built `RegseqModel`. Bridge-side type construction is
fully owned by stage 3 (Ty variants), stage 11 (FnSig
content), and stage 12 (Item content + DefPath rendering).

## Open items

(none yet -- created when stage 13 opens)
