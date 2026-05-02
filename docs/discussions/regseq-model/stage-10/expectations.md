# Expectations on stage 10 (incoming)

Stage 10 covers MIR basic blocks and introduces the `BlockId`
typed-ID.

The convention is described in `../README.md` under "Forward
expectations live in `stage-N/expectations.md`."

## From stage 1

- **Define `BlockId` via `newtype_index!`.** Stage 1 forward-
  declared `LocalId` and `BlockId` only conceptually (the IDs
  themselves are stage-10/11 concerns). Stage 10 introduces
  `BlockId` using the existing macro at
  `crates/regseq-model/src/idx.rs`.

- **`newtype_index!` const-body arm.** The macro currently
  accepts only empty-body invocations (`pub struct $Name {}`).
  Stage 10 needs a const-body arm so the body-local convention
  `BlockId::START_BLOCK = BlockId::from_u32(0)` (per the spec's
  "Body-local ID constants" section) can be a const associate
  declared inline:

  ```
  newtype_index! {
      pub struct BlockId {
          const START_BLOCK = 0;
      }
  }
  ```

  The `from_u32(0)` form is already const, so the macro change
  is purely about parsing additional grammar in the macro
  invocation. Plan-agent flagged this as a deferred follow-up
  during step 1; stage 10 lands it. The current empty-body arm
  must keep working unchanged.

- **`bbN` sigil rendering.** Add `BlockId`-rendering to
  `pretty.rs`. The spec sigil is `bbN` (e.g., `bb0`, `bb1`).
  Currently `pretty.rs` notes this as deferred next to
  `LocalId`'s `_N` sigil.

- **Niche optimization for `Option<BlockId>`.** When stage 10
  adds `BlockId`, append the corresponding `Option<BlockId>` and
  `BlockId` 4-byte size assertions to `crates/regseq-model/src/lib.rs`.

## Bridge concerns

- **Block ID assignment in reverse-postorder.** The cleanup-pass
  whitelist at `crates/regseq-driver/src/callbacks.rs` runs
  `ReorderBasicBlocks` (reverse-postorder block IDs) before MIR
  reaches the bridge. Stage 10 inherits this ordering when
  assigning `BlockId`s.
