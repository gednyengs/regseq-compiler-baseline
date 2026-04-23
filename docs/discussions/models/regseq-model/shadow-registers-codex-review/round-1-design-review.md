**Q1**

Verdict: has-gaps.

- Full-register reads are unspecified. The design names `read_field`, but current `Mmio` also has `read(REG)` as a first-class intrinsic at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq/src/mmio.rs:37`, and `modify` currently calls `self.read(_reg)`, not `read_field`, at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq/src/mmio.rs:77` and `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq/src/mmio.rs:84`. Example: `self.regs.modify(imsc::REG, imsc::RXIM.val(1))` is only shadow-aware if full `read(imsc::REG)` is shadow-aware too.

- Mixed read-side-effect registers are not covered. Current register metadata only has coarse `ReadWrite`, `ReadOnly`, `WriteOnly`, and `Reserved` access kinds at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/register.rs:105`. Example: a `ReadWrite` `STATUS_CTL` register with `ENABLE` as normal RW and `ERR` as read-clear-on-read cannot safely be shadowed as a whole register unless the DSL can express and reject that `ERR` read effect.

- `force_read` cache behavior needs exact ordering. Existing `RegseqIntrinsic` has only `RegisterRead`, `RegisterReadField`, `RegisterWrite`, and `RegisterWriteField` for register accesses at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/ir/intrinsic.rs:20`; no force-read intrinsic exists. Example: if `force_read(imsc::REG)` races with an ISR write, the design must say whether the cache update is atomic, lock-protected, or simply last-writer-wins.

- Future atomic bit ops need semantics now. Current write values carry a mask/value pair at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq/src/registers.rs:130`, and `write_field` is the masked write intrinsic at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq/src/mmio.rs:63`. Example: future `set_bit(imsc::RXIM)` should be specified as shadow-RMW-plus-hardware-write, not hardware atomic bit-set, or the shadow can diverge.

- Shadow ownership is underspecified. Current model separates descriptor identity from block fields: descriptors hold bitfields at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/register.rs:21`, while block fields hold offset/access at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/register.rs:88`. A shadow is per MMIO register instance, so the design should attach it to the register-block field, not a reusable descriptor.

**Q2**

Verdict: has-gaps.

`register_block!` currently parses only a top-level `#[endian = ..., access_width = ...]` attribute at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-macros/src/register_block.rs:201`, then parses each entry as `name: AccessKind { ... }` at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-macros/src/register_block.rs:164`. It emits a `#[rseq::register_block(...)]` struct plus per-register modules with `OFFSET`, `REG`, field constants, and `RegisterOf` impls at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-macros/src/register_block.rs:330`.

Current construction reads only block-level endian/access-width at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/registers.rs:44`, classifies field wrapper types into access kinds at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/registers.rs:107`, and builds `RegisterBlockField` without shadow/reset metadata at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/registers.rs:62`. The model also lacks a shadow slot: `RegisterBlockField` currently has only `name`, `access_kind`, `descriptor`, `byte_offset`, and `array_count` at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/register.rs:89`.

Intrinsic metadata currently carries block, offset, width, and optional field metadata only at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/ir/intrinsic.rs:161`. Register metadata resolution attaches exactly those fields at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/register_metadata.rs:442`. So shadow needs to flow into the register model and/or intrinsic metadata before lowering; backend-only rewriting would make earlier verification and semantic IR lie about hardware reads.

`call_lowering.rs` already distinguishes `RegisterWrite` from `RegisterModify` at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/semantic_lowering/call_lowering.rs:47` and `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/lowering/semantic_lowering/call_lowering.rs:57`. That distinction remains necessary because shadowed write-field means “compute full cache value and write hardware,” while modify means “preserve unmasked bits from cache.” Existing Linux helpers implement hardware RMW by reading hardware at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_module.rs:293`, so a shadowed modify cannot keep using the current generic `reg_modify`.

The auto-generated shadow field should bypass the user-data path entirely. Current driver construction treats unclassified fields as user data and rejects non-`Option<T>` at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/driver.rs:216` and `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/driver.rs:249`. Current Linux struct fields are derived from MMIO, IRQ, clocks, properties, and serial port at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/planner.rs:140`, then emitted from `shared_struct_fields` at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emitter.rs:152`; shadow state needs its own compiler-managed state list in `RegseqModel` or the lowering plan.

**Q3**

Verdict: insufficient.

Rejecting WO is necessary: `WriteOnly` is a current access kind at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/register.rs:113`, and the fixture’s `icr` is declared `WriteOnly` at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/registers.rs:206`. But W1C is not represented as a first-class access kind today; the fixture documents ICR W1C in comments at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:121`, not in the model.

Reserved bits are also not representable enough to validate. IMSC declares bits 0-10 only at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/registers.rs:161`; reserved bits 11-31 are absent from the descriptor. The current manual helper writes only `0x07ff` active bits and relies on zero for reserved bits at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:103`. The compiler can check literal width, but it cannot check datasheet reset/reserved policy unless that policy enters the DSL.

Hardware-state-dependent reset values need an escape hatch. Mandatory literal `reset_value` is good for PL011 IMSC, but wrong for fused/configuration registers; those should either be ineligible for reset-seeded shadows or require an explicit force-read/bootstrap policy.

**Q4**

Verdict: sound, with a stronger story if scoped tightly.

This strengthens the paper story because the current fixture encodes a semantic hardware invariant as ordinary mutable Rust state: `imsc_shadow: u32` at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:72`, `commit_imsc_shadow` at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:108`, and repeated manual updates at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/tests/fixtures/pl011-full-spec/src/driver.rs:236`. Making this a register property gives the compiler something it can validate and emit visibly in C.

It generalizes to write mirrors for interrupt masks, enable registers, mode registers, and software-owned control words. It does not generalize cleanly to counters, FIFOs, status registers, read-clear fields, or hardware-updated state unless invalidation and refresh semantics are modeled.

The answer to “why not let the author write the field manually?” is concrete: current construction rejects the bare `u32` shape at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/driver.rs:249`, and `Option<T>` user data is explicitly the current contract at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/resources.rs:306`. A compiler-managed shadow is not ordinary user data; it is part of the register semantics.

**Q5**

Verdict: medium scope, high integration risk.

1. Surface grammar and macro metadata, 1-2 days: add per-register shadow/reset syntax to the macro path that currently only accepts register entries at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-macros/src/register_block.rs:164`.

2. Model and serialization, 1-2 days: add shadow/reset metadata near `RegisterBlockField`, whose current shape lacks it at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/register.rs:89`.

3. Construction propagation, 1-2 days: carry macro-emitted metadata through `build_register_block`, currently building fields at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/registers.rs:58`, and into `RegisterIndex`, currently keyed by marker type with no shadow info at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/register_metadata.rs:42`.

4. Validation, 2-4 days: current access validation only checks read/write capability at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/register_metadata.rs:569`; real shadow validation needs side-effect/reserved/reset policy.

5. Intrinsic surface, 1 day: add `force_read` to `Mmio`, the registry, and `RegseqIntrinsic`, whose accepted canonical names are currently fixed at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/ir/intrinsic.rs:96`.

6. Lowering and emission, 3-6 days: shadow reads/writes must affect semantic IR and Linux emission. Risk is highest here because current emitters lower `RegWrite` and `RegModify` directly to hardware helpers at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_semantic.rs:637` and `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_semantic.rs:645`, and there is also a behavior emitter path at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emit_behavior.rs:106`.

Realistic total: about 8-15 engineering days, depending on how much side-effect validation is included now.

**Q6**

Verdict: land before Linux reference C, after amendments.

Because the feature changes the generated C driver struct and register helper behavior, co-authoring the Linux golden reference first would create expected churn. The current C struct emission comes from planned shared fields at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-backends/src/linux/emitter.rs:152`, and this proposal explicitly adds a new emitted state field. Also, the current fixture’s manual bare `u32` shadow is exactly what trips construction today at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-driver/src/construction/driver.rs:216`. Do the design amendments, land the minimal IMSC shadow path, then write the Linux reference against the final shape.

**Q7**

Verdict: explicit block-field shadow is better than the stated descriptor attribute.

The missed better alternative is not compiler inference; inference would be unsound because current metadata cannot express read-clear, W1C, hardware-updated, or reset-source facts. The better alternative is to make this explicitly a register-block-field property, not a reusable descriptor property, because the current model already puts offset/access on `RegisterBlockField` at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/register.rs:88`, while descriptors are reusable bitfield layouts at `/Users/nyengele/Documents/sekekama/regseq-compiler-asplos/crates/regseq-model/src/model/register.rs:15`.

Worth comparing: a narrower `write_mirror(reset_value = ...)` name, a generated compiler-managed state field category, and an explicit `force_read`/bootstrap policy. I would not pursue static inference for Stage 1.

OVERALL-DESIGN-VERDICT: adopt-with-amendments.

The core idea is sound and paper-useful, but the current proposal under-specifies full-register reads, concurrency, force-read ordering, special read/write side effects, reserved/reset validation, and where compiler-managed state lives in the model.

Amendments:
- Attach shadow metadata to register-block fields, not reusable descriptors.
- Define shadow semantics for `read`, `read_field`, `write`, `write_field`, `modify`, and `force_read`.
- Add explicit concurrency/ISR ordering rules or require caller-held synchronization.
- Add side-effect eligibility metadata before claiming W1C/read-clear rejection.
- Represent compiler-managed shadow state separately from user data.
- Lower shadow semantics before backend emission, not as a Linux-only rewrite.
