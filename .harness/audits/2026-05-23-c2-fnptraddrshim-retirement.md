# C2 (FF-013, FnPtrAddrShim) — RETIRED by reclassification; FF-013 authored

**Status**: RESOLVED (2026-05-23). The F68 audit's C2 proposal is
**retired as mis-scoped** (no bridge code change). Its target row,
**FF-013**, is authored as a passing sidecar row with **zero bridge
changes** via a direct `<fnptr as FnPtr>::addr` snippet. Verified at
HEAD `333dfaf` on the pinned `nightly-2026-02-01` toolchain. Mode A:
Total=141, Pass=141, Fail=0.

This is the **second F68 proposal retired for mis-scoping** after C1
(OP-003-I) — the same lesson: const-related audit proposals' fix-hooks
are frequently wrong; verify the *mechanism* against HEAD + the bridge
model + an empirical run before drafting.

---

## 1. What C2 proposed

Per `.harness/audits/2026-05-21-post-f4-triage.md:312-347`, C2
("Accept fn-pointer-provenance allocations (FnPtrAddrShim)") proposed
to fix the bridge's const-translation allocation-provenance path so it
**accepts `GlobalAlloc::Function`-provenance allocations** (rejection
site `crates/regseq-driver/src/bridge/const_.rs:526-542`, where only
`GlobalAlloc::Memory` and `GlobalAlloc::Static` are handled and
`Function` falls into the `_` reject arm), with the stated mechanism
that "the auxiliary-instance scan would then materialize the
FnPtrAddrShim downstream."

The proposal originated from the F4 escalation
(`.harness/expectations/regseq-driver-67.md:337-351`), which tried
three fn-pointer **comparison** shapes; the only one reaching a
fn-ptr-provenance path, `PartialEq::eq(&fp, &(target as fn()))`, was
rejected with `UnsupportedConst: allocation provenance over
Function {...}`. From that single rejection the escalation inferred a
"const-eval-over-fn-pointer-provenance" blocker.

## 2. Why C2 is mis-scoped (empirical, HEAD)

`FnPtrAddrShim` is the rustc instance kind for `<fnptr as FnPtr>::addr`
(the `FnPtrAddr` lang item), per
`rustc_ty_utils/src/instance.rs:283-292`. It is **not** a const-eval or
allocation-provenance construct.

Five candidate snippets were bridged in-process (bridge-matching config:
`mir_opt_level=0` + `always_encode_mir`, `-C panic=abort`,
`--crate-type rlib`), reading the flavor of every
`InstanceKind::Function` in the resulting `RegseqModel`:

| Snippet | Bridge outcome | FnPtrAddrShim in model? |
| ------- | -------------- | ----------------------- |
| `a == b` (two `fn()`) | Ok, flavors `[Item]` | no — `==` lowers to a primitive `BinOp::Eq`, no call |
| `core::ptr::fn_addr_eq(a, b)` | Ok, flavors `[Item]` | no |
| `PartialEq::eq(&a, &b)` (runtime refs, **no const**) | Ok, flavors `[Item, Item]` | **no** |
| `PartialEq::eq(&fp, &(g as fn()))` (promoted const) | **Rejected**: `allocation provenance over Function {...::g}` | n/a |
| `<fn() as FnPtr>::addr` direct, `#![feature(fn_ptr_trait)]` | Ok, flavors `[FnPtrAddrShim, Item]` | **yes** |

The decisive row is `PartialEq::eq(&a, &b)` with **no const at all**:
it bridges to `[Item, Item]` with **no** `FnPtrAddrShim`. Since there
is no const allocation in that shape, removing the const-provenance
rejection (C2's entire premise) **cannot** surface the flavor. C2 is
therefore neither necessary nor sufficient for FF-013.

### 2a. Mechanism correction (caught by Codex)

An earlier draft of this finding claimed core's fn-pointer `PartialEq`
"never calls `FnPtr::addr`, it uses raw-pointer compare." **That is
wrong.** Codex (session reviewed 2026-05-23) verified against the
`rust-src` library tree (shipped at
`~/.rustup/toolchains/nightly-2026-02-01-*/lib/rustlib/src/rust/library/`,
distinct from the `rustc-src` compiler tree):

- `core/src/ptr/mod.rs:2490` — `fn_addr_eq` body is `f.addr() == g.addr()`.
- `core/src/ptr/mod.rs:2532` — `PartialEq for FnPtr` is `self.addr() == other.addr()`.

And `rustc -Zprint-mono-items` shows `<fn() as FnPtr>::addr -
shim(fn())` **is** mono-collected for the comparison shapes. So at the
rustc level, comparison *does* route through `FnPtr::addr`.

The correct mechanism: **the bridge's model-building instance walk is
shallower than rustc's monomorphization closure.** It does not surface a
`FnPtrAddrShim` *instance* for `.addr()` calls nested inside library
bodies (`fn_addr_eq` / `PartialEq::eq`), but it does surface one for a
direct `.addr()` call in a walked (user-crate) body. The conclusion is
unchanged — only the explanation is. (Whether the shallow walk is
correct for the bridge's purposes is a separate question, not relevant
to FF-013.)

## 3. How FF-013 is covered (zero bridge changes)

`crates/bridge-coverage-runner/snippets/FF-013.rs`:

```rust
#![no_std]
#![feature(fn_ptr_trait)]
use core::marker::FnPtr;
pub fn f(p: fn()) -> *const () {
    p.addr()
}
```

The bridge already handles the flavor: the `FnPtrAddrShim` instance is
registered with body absent via the deferred arm at
`crates/regseq-driver/src/bridge/body/materialise.rs:246-269` (the same
"recognized, no shim builder, body-absent" treatment given to the other
non-`Item`/non-`DropGlue` shim flavors). The model validates OK. The
runner's `FunctionInstanceFlavor=FnPtrAddrShim` predicate
(`crates/bridge-coverage-runner/src/inspect.rs:629-642`) requires only
flavor presence, so the row PASSES.

`#![feature(fn_ptr_trait)]` is consistent with corpus convention — 32
existing snippets carry feature gates (`core_intrinsics`, `coroutines`,
`never_type`, `extern_types`, `f128`/`f16`, `thread_local`, ...).

## 4. The const-provenance rejection is a real but SEPARATE gap

`const_.rs:526-542` genuinely rejects `GlobalAlloc::Function`-provenance
allocations (and any provenance that is not `Memory`/`Static`). This
would matter for fn pointers **embedded in consts/statics** — e.g. a
`static DISPATCH: [fn(); N]` table or a `const FP: fn() = g`. That is a
legitimate future row/task candidate if a driver-relevant shape needs
it, and it must **not** be conflated with FnPtrAddrShim coverage. It is
explicitly out of scope here.

## 5. Disposition

- **C2 RETIRED** — removed from the active escalation queue; no bridge
  change filed. (FF-013 was the only escalation C2 covered.)
- **FF-013 `uncovered` → `covered`** in the matrix
  (`docs/discussions/regseq-model/bridge-coverage-matrix.md:406`) and as
  a committed sidecar row (`matrix_metadata/rows.toml`,
  `snippets/FF-013.rs`). Sidecar 140 → 141.
- Const-provenance-in-const/static noted as a separate deferred gap.
