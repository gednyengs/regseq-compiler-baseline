# The RegseqModel input contract

**What a backend consumes.** This document specifies the exact API surface a
backend reads out of `regseq-model` to lower the five in-scope constructs. It is
written from the backend's point of view: *given a post-mono `&RegseqModel`,
which fields do I traverse, how do I recognize each construct, and what does each
carrier hand me.*

Everything here is a **read contract** — `RegseqModel` is immutable to a
backend, and its *procedural* spine is OS-agnostic, but the model is **not**
target-agnostic: the root carries the bridge-recorded ABI facts
(`target: TargetInfo`, §1) that a backend must read for pointer-width and
endian-sensitive lowering. The reference backend `generic_c` consumes this
model surface **plus one out-of-band input** — the non-model `BackendContext`
(§1.1); the per-bank machinery that turns it into C is classified in
[`agnostic-core-vs-os-shell.md`](./agnostic-core-vs-os-shell.md), and the
primitive-by-primitive census is [`primitive-inventory.md`](./primitive-inventory.md).

> All types named below are re-exported from the crate root
> (`crates/regseq-model/src/lib.rs:49-119`), so a backend imports them as
> `regseq_model::{...}`. Citations are to the **declaration** line in the
> defining module.

---

## 1. The model root: what you iterate

`RegseqModel` (`crates/regseq-model/src/model.rs:54`) is the whole input. A
backend touches a small, stable subset of its public fields:

| field | line | what a backend uses it for |
|---|---|---|
| `target: TargetInfo` | model.rs:61 | the **target ABI facts** — `pointer_width: u8` (target.rs:51) and `endian: Endian` (target.rs:53; struct `TargetInfo` target.rs:47). The reference backend reads `pointer_width` for pointer-sized integer lowering (`ptr_bits`, `generic_c/body_emit.rs:370-372`) and `endian` for `Const::Bytes` byte-image reconstruction (`body_emit.rs:3209`, `:4399`). **Not optional** — a backend that skips this emits wrong code on a non-host target |
| `instances: IndexVec<InstanceId, Instance>` | model.rs:100 | the **primary work list** — every function / static / const / aggregate to emit, in deterministic `InstanceId` order |
| `types: TyInterner` | model.rs:72 | resolve a `TyId` to a `Ty` (`model.types[ty]`); the type system you spell into C |
| `sigs: FnSigInterner` | model.rs:76 | resolve a `FnSigId` to a `FnSig` (`model.sigs.resolve(id)`) |
| `consts: ConstInterner` | model.rs:74 | resolve a `ConstId` to a `Const` (static inits, operand constants) |
| `strings: StringInterner` | model.rs:67 | resolve a `StringId`/`Symbol` to `&str` (`model.strings.resolve(id)`) — names, match tokens, link sections |
| `items: IndexVec<ItemId, Item>` | model.rs:79 | per-item metadata: linkage, inline hint, `is_result`, `opaque_handle_eligible`, the `DefPath` (`model.items[item]`) |
| `sig_c_abi_lowerings: IndexVec<FnSigId, Option<CAbiLowering>>` | model.rs:97 | the C-ABI side table (boundary vs body-local types); bounds-check before indexing (hand-built models may omit it) |
| `domain_attrs: DomainAttrTables` | model.rs:118 | **the domain-attribute layer** — how you recognize register/config/log/framework intrinsics (§4) |
| `device_lifecycle: DeviceLifecycleTables` | model.rs:122 | **the lifecycle scenario model** — drivers, descriptors, callbacks, resource bindings (§5, §6) |

A backend emitting a single translation unit is, structurally, a fold over
`model.instances` plus two side tables (`domain_attrs`, `device_lifecycle`) that
re-key into it. The reference backend's outermost loop is exactly this
(`generic_c/mod.rs:884-924` for discovery; the instance-symbol fold
`mod.rs:1027-1056`; and the per-instance banks `:2199-2237`, `:2276-2324`).

### 1.1 The out-of-band input: `BackendContext`

The model is **not the whole emission input**. The reference emission is pure
over `(model, BackendContext)` — the production entry point is
`emit_generic_c_with_context` (`generic_c/mod.rs:180-185`), threading into
`emit_generic_c_inner(model, adapter, options, platform)`
(`mod.rs:237-242`). `BackendContext` (`generic_c/backend_options.rs:129`)
bundles two non-model inputs:

- `options: BackendOptions` (`backend_options.rs:74`) — harness/codegen knobs;
  today the lifecycle scenario selection (§5).
- `platform: PlatformIdentity` (`regseq-model/src/target.rs:114`; a model-crate
  type but **not** a `RegseqModel` field — it is sourced from the platform
  file's `[platform]` table and passed alongside the model) — the os/board/arch
  identity. `generic_c` only *traces* it as a one-line header comment
  (`mod.rs:298-305`); a real OS backend selects its framework from it, and that
  framework selection itself remains **recognized-not-lowered** (§9).

An implementer following only the §1 table would miss both; a new backend must
plan its equivalent of this context from the start.

---

## 2. Items, instances, signatures, bodies

### 2.1 Instance — the unit of emission

`Instance` (`crates/regseq-model/src/instance.rs:134`) carries:

- `item: ItemId` (instance.rs:136) — back-reference to the owning `Item` (linkage, attrs).
- `generic_args: GenericArgs` (instance.rs:138) — the monomorphization key; drives the internal-symbol suffix (§2.4).
- `emission_name: StringId` (instance.rs:140) — the bridge-minted, generic-agnostic `DefPath.rendered`; **not** a valid C symbol — a backend derives the symbol from it (§2.4).
- `kind: InstanceKind` (instance.rs:144).

`InstanceKind` (`instance.rs:71`) is the dispatch:

| variant | line | meaning |
|---|---|---|
| `Function(FunctionInstanceData)` | instance.rs:73 | a function; carries the signature + optional body |
| `Static { ty: TyId, init: Option<ConstId> }` | instance.rs:76 | a static; `init: None` ⇒ declaration only |
| `Const { ty, value: ConstId }` | instance.rs:84 | a const; materializes at use sites, has no standalone symbol |
| `Struct { layout: TyId }` / `Enum { layout }` / `Union { layout }` | instance.rs:92/98/104 | a nominal aggregate **type tag** (a different C namespace from functions/statics) |

`FunctionInstanceData` (`instance.rs:28`): `fn_sig: FnSigId` (`:30`),
`body: Option<Body>` (`:32`), `flavor: FunctionInstanceFlavor` (`:34`),
`virtual_idx: Option<u32>` (`:36`). Access the body via
`instance.kind.as_function()?.body()` (the reference backend's accessor pattern,
e.g. `generic_c/mod.rs:912`).

### 2.2 Item — per-item emission metadata

`Item` (`crates/regseq-model/src/item.rs:18`) fields a backend reads:

- `linkage: Linkage` (item.rs:31) — `Default` / `NoMangle` / `Foreign` / `Weak` (item.rs:149-157). **External** linkage (`NoMangle`, `Foreign`) is an ABI contract: a stable, suffix-free symbol (§2.4).
- `link_section: Option<StringId>` (item.rs:38), `inline_hint: InlineHint` (item.rs:45) — section / inline qualifiers (`generic_c/mod.rs:1453-1472`).
- `is_result: bool` (item.rs:101) — the **compiler-certified** `core::result::Result` flag (driver-91). The entry/RegseqError classifier requires it (§8); a look-alike enum that merely *renders* as `Result` has `is_result == false` and must fail loud.
- `opaque_handle_eligible: bool` (item.rs:90) — emit forward-decl-only, used behind `struct X *` (IT-14).
- `kind: ItemKind` (item.rs:22; variant `Static { mutability }` :128, in `enum ItemKind` :124) — `Static { mutability }` distinguishes `static` (const) from `static mut`.

### 2.3 FnSig and the C-ABI seam

`FnSig` (`crates/regseq-model/src/fn_sig.rs:15`): `inputs: Vec<TyId>` (`:18`),
`output: TyId` (`:22`), `abi: FnAbi` (`:25`; `Rust`/`C`, `ty.rs:211`),
`c_variadic: bool` (`:31`), `safety: Safety` (`:35`).

For an `abi == C` signature, the **effective emitted type** of a position may
differ from the body-local type. `CAbiLowering` (`cabi.rs:37`) carries
`inputs: Vec<CAbiShape>` and `output: CAbiShape`, where `CAbiShape` (`cabi.rs:9`)
is `Direct` (boundary type *is* the body type) or
`Substituted { emit_as: TyId, reason: SubstReason }` (a `repr(transparent)` /
tag-scalar boundary requiring prologue/call-site marshalling). The reference
backend resolves the effective type with a bounds-checked side-table lookup
(`generic_c/mod.rs:1330-1362`); a position with no lowering entry uses the raw
`FnSig` type. **A new backend must honor this seam** to spell signatures and
call sites correctly.

### 2.4 Symbol derivation (a backend responsibility)

`emission_name` is not a C symbol. The reference derivation
(`generic_c/naming.rs:279-287`, `sanitize` `:202`) is:

- **External** linkage (`NoMangle` / `Foreign`): `sanitize(emission_name)` with
  **no** generic suffix — stable across monomorphizations (the link-time ABI
  name).
- **Internal** linkage (`Default` / `Weak`): `sanitize(emission_name) +
  generic_suffix(generic_args)` — distinct monomorphizations get distinct
  symbols. A `_clash_<n>` net resolves residual collisions
  (`generic_c/mod.rs:1043-1054`).

This is a *convention*, not a model field; any C-targeting backend needs an
equivalent. See [`agnostic-core-vs-os-shell.md`](./agnostic-core-vs-os-shell.md)
§ *Bank: name tables*.

---

## 3. The procedural core: bodies, blocks, statements

A `Body` (`crates/regseq-model/src/body.rs:24`) is MIR-shaped:

- `basic_blocks: IndexVec<BlockId, BasicBlock>` (body.rs:27)
- `local_decls: IndexVec<LocalId, LocalDecl>` (body.rs:33) — `LocalDecl.ty` (`:114`), `mutability` (`:118`), `debug_name` (`:126`). Locals `1..=arg_count` are the parameters.
- `arg_count: usize` (body.rs:37)

`BasicBlock` (`basic_block.rs:36`): `statements: Box<[Statement]>` (`:41`) +
`terminator: Terminator` (`:45`).

A backend lowers each block as a labeled C target (`bb_<n>`) and translates:

**`StatementKind`** (`statement.rs:34`):

| variant | line |
|---|---|
| `Assign(Place, Rvalue)` | statement.rs:40 |
| `SetDiscriminant(Place, VariantIdx)` | statement.rs:47 |
| `Intrinsic(NonDivergingIntrinsic)` — `Assume` / `CopyNonOverlapping` | statement.rs:54 |

**`TerminatorKind`** (`terminator.rs:64`):

| variant | line | note |
|---|---|---|
| `Goto(BlockId)` | terminator.rs:66 | |
| `SwitchInt { discr, targets: SwitchTargets }` | terminator.rs:70 | `SwitchTargets` = `cases: [(u128, BlockId)]` + `otherwise` (`:128`) |
| `Return` | terminator.rs:79 | |
| `Unreachable` | terminator.rs:82 | |
| `Call { callee: CallCallee, args, destination, target }` | terminator.rs:85 | **the domain-intrinsic interception point** (§4, §7) |
| `Assert { cond, expected, msg: AssertKind, target }` | terminator.rs:97 | `AssertKind` (`:141`) maps to the prelude panic catalog |

**`Rvalue`** (`rvalue.rs:19`): `Use` / `Repeat` / `Ref` / `RawPtr` / `Cast` /
`BinaryOp` / `UnaryOp` / `Discriminant` / `Aggregate` (`:23-72`), with `BinOp`
(`:117`), `UnOp` (`:174`), `CastKind` (`:77`), `AggregateKind` (`:185`).
**`Operand`** (`operand.rs:30`): `Copy(Place)` / `Move(Place)` /
`Constant(ConstId)`. **`Place`** (`place.rs:30`): `local: LocalId` +
`projection: Box<[PlaceElem]>`, with `PlaceElem` (`:70`) =
`Deref` / `Field` / `Index` / `ConstantIndex` / `Subslice` / `Downcast`.

This procedural spine is **OS-neutral** — it is the same for every backend, and
in the reference backend it is the reusable "core" half of body lowering. The
five constructs ride *on top* of it: **three** of them lower through a `Call`
whose callee the backend intercepts (§4) — a fourth intrinsic family,
`PollUntil`, is recognized in the model but has **no** emit arm — and the
lifecycle is synthesized *around* the bodies (§5).

---

## 4. The domain-attribute layer (how you recognize a construct)

This is the heart of recognition. `DomainAttrTables`
(`crates/regseq-model/src/domain_attr.rs:14`) has three keyings:

| table | line | key | use |
|---|---|---|---|
| `item_attrs: IndexVec<ItemId, Vec<AttrData>>` | domain_attr.rs:19 | `ItemId` | **the one a backend reads** — intrinsic + framework markers on an item |
| `field_attrs: BTreeMap<FieldPath, Vec<AttrData>>` | domain_attr.rs:26 | `FieldPath {item, variant, field}` (`:45`) | field-level attrs (sparse) |
| `span_attrs: BTreeMap<Span, Vec<AttrData>>` | domain_attr.rs:33 | `Span` | reserved |

`AttrData` (`domain_attr.rs:62`):

- `Intrinsic(IntrinsicKind)` (`:64`)
- `FrameworkType(FrameworkType)` (`:68`)

`IntrinsicKind` (`domain_attr.rs:88`) — the four recognized intrinsic families:

| variant | line | construct |
|---|---|---|
| `Log(LogLevel)` | domain_attr.rs:90 | diagnostics-logging — `LogLevel` Debug/Info/Warn/Error (`:141`) |
| `Mmio(MmioOp)` | domain_attr.rs:93 | register-access — `MmioOp` Read/ReadField/Write/WriteField/Modify (`:125`) |
| `PollUntil` | domain_attr.rs:95 | register-access — **recognized, not lowered** |
| `Config(ConfigOp)` | domain_attr.rs:100 | bus-resource-management — `ConfigOp` Read/Write (`:110`) |

`FrameworkType::RegseqError` (`domain_attr.rs:79`) marks the authored error enum
(§8). `IntrinsicKind::from_name(Symbol)` (`domain_attr.rs:159`) is the
dash/underscore-form name table (`"log-debug"`, `"mmio_read"`, `"read_config"`,
…) the bridge uses on the way in; a backend reads the already-parsed
`IntrinsicKind`, not the name.

### The recognition pattern

A backend does **not** scan statements for intrinsics. It intercepts at the
`Call` terminator, keyed by **instance identity**: a `CallCallee::Direct(inst)`
whose owning item carries the attr. The reference backend wraps this in the
`instance_intrinsic(inst)` accessor and dispatches in one place
(`generic_c/body_emit.rs:2100-2125`):

```
Direct(inst) + IntrinsicKind::Log(level)  -> emit_diag_log   (body_emit.rs:2101-2104)
Direct(inst) + IntrinsicKind::Mmio(op)    -> emit_mmio       (body_emit.rs:2113-2116)
Direct(inst) + IntrinsicKind::Config(op)  -> emit_config     (body_emit.rs:2122-2125)
```

These are the only **three** lowerable call-intercept arms. The fourth
recognized intrinsic family, `PollUntil` (`domain_attr.rs:95`,
`from_name` arm `:167-172`), has **no** emit arm in `call()`
(`body_emit.rs:2100-2125`) — it is recognized-not-lowered, and the
register-access surface explicitly forbids claiming it
(`docs/discussions/register-access/register-access-surface-convergence.md:424-427`).

An `Indirect` callee can never satisfy recognition — recognition is by identity,
not by signature shape. **This recognition is the OS-neutral half; what each arm
*emits* is the OS-specific half** (see the seam doc).

---

## 5. The lifecycle scenario model

Unlike the three call-intercepted intrinsics, the device lifecycle is **not**
call-intercepted (the harness verbs were removed, RD-1; `body_emit.rs:2126-2128`,
`:2416-2418`). It is synthesized by the backend *from the side table*, around the
emitted callback bodies.

`DeviceLifecycleTables` (`device_lifecycle.rs:27`): `drivers: BTreeMap<ItemId,
DriverLifecycle>` (`:30`) — keyed by **driver-struct `ItemId`**, deterministic
emission order. `DriverLifecycle` (`:36`) =
`descriptor: DriverDescriptor` (`:38`) + `handle: DriverHandle` (`:40`) +
`ops: LifecycleOps` (`:42`).

**`DriverDescriptor`** (`device_lifecycle.rs:47`):

- `name: StringId` (`:49`) — the registration/trace name.
- `match_set: Vec<MatchEntry>` (`:51`) — the typed device-match set; `MatchEntry`
  (`:110`) is `Devicetree`/`Pci`/`Amba`/`Acpi`/`Usb`/`I2c`/`Spi` (`:112-129`).
- `device_class: ClassTag` (`:53`) — Generic/Serial/Network/Other (`:135`); **recognized, not lowered**.
- `bus_type: Option<BusType>` (`:61`) — the BRM registration selector (§6); `None` ⇒ a non-bus driver.
- `dma_present: bool` (`:70`) — placeholder gate for `set_bus_master` (defaults `false`; out-of-scope DMA fact).

**`LifecycleOps`** (`device_lifecycle.rs:160`) — five optional callbacks, each an
`Option<InstanceRef>` resolving to the emitted body's instance, `None` when the
author did not override (the trait default is a no-op):

| field | line |
|---|---|
| `on_init` | device_lifecycle.rs:162 |
| `on_deinit` | device_lifecycle.rs:164 |
| `on_surprise_remove` | device_lifecycle.rs:166 |
| `on_init_failure` | device_lifecycle.rs:168 |
| `on_system_shutdown` | device_lifecycle.rs:170 |

An `InstanceRef` (`instance_ref.rs:10`) is a **content-addressed** callback
reference (def-path hash + args key + kind rank) that survives instance
permutation; a backend resolves it to an `InstanceId` via the reference backend's
`resolve_reloc` (`generic_c/body_emit.rs:4965-4973`, invoked via the
`resolve_sym` closure `:5103-5113`).

### Scenarios are a backend concept, not a model type

The model carries *which callbacks exist*. **Which scenario(s) to drive** is a
backend option: `LifecycleScenario` (`generic_c/backend_options.rs:24`) —
`Orderly` / `Surprise` / `InitFail` / `Shutdown` — selected via
`BackendOptions` (`backend_options.rs:74`, default = all four) from a
`backend.lifecycle.scenario` CSV param (`:101`). A new backend reuses the model's
`LifecycleOps` but **defines its own scenario semantics** against its OS's probe
/ remove / shutdown model. The reference scenario synthesis and its canonical
ordering are in `generic_c/body_emit.rs:5088-5382`.

### The managed steps and their ordering

Within a scenario the reference backend synthesizes managed steps that have **no
model carrier** — they are backend-owned (`create`, `associate`, `teardown`, the
`on_init -> Err` transition marker) — interleaved with the model-driven ones
(`register` from the descriptor; `preamble`/`postamble` per
`handle.resource_fields`). Forward order at bring-up, reverse at teardown
(`body_emit.rs:5197-5217`, `:5391-5407`). See §6 for the BRM ops slotted into
this same preamble.

---

## 6. The BRM binding model and the `map_region` seam

`DriverHandle` (`device_lifecycle.rs:177`) carries
`resource_fields: Vec<ResourceField>` (`:180`) and
`user_data_fields: Vec<UserDataField>` (`:182`, recognized-not-lowered).

**`ResourceField`** (`device_lifecycle.rs:187`): `name: StringId` (`:189`),
`kind: ResourceKind` (`:191`; `Mmio` `:256`), `binding: MmioBinding` (`:200`).

**`MmioBinding`** (`device_lifecycle.rs:209`) — the L1↔L2 region join result:

| field | line | meaning |
|---|---|---|
| `id: Option<StringId>` | :214 | the `#[rseq::mmio(id="…")]` resource id |
| `bar_index: Option<u8>` | :217 | L1-preferred BAR |
| `bar_type: BarType` | :220 | `Mem` (default) `:247` / `Io` `:249` (`BarType` enum `:244`; `Io` recognized-not-lowered) |
| `effective_bar: Option<u8>` | :223 | **`L2.bar ?? L1.bar`** — the resolved BAR that keys the region |
| `base: Option<u64>` | :226 | L2-resolved region base |
| `size: Option<u64>` | :228 | L2-resolved region size |

`MmioBinding::is_resolved()` (`:236-238`) is `base.is_some() && size.is_some()` —
the **gate** for the entire BRM preamble block.

### The keystone composition

When a driver has a resolved binding, the reference backend synthesizes the four
compiler-managed BRM ops **into the device-lifecycle preamble**, in a fixed
canonical order, then hands the resolved `(base, size)` to register-access's
`map_region` (`generic_c/body_emit.rs:5244-5285`):

```
  register → create → associate → preamble(per resource)        [DL managed steps, §5]
  ── if any binding is_resolved: ──
    enable_device          (PCI-capable bus only; is_pci_like)   body_emit.rs:5256-5259
    claim_regions(bar)     (per region-bearing binding, EVERY bus) :5260-5266
    set_bus_master         (PCI && dma_present — dormant)        :5267-5270
    get_resource_handle    (unconditional)                       :5271-5272
    map_region(base,size,bar)  ◄── register-access consumes the BRM-resolved (base,size)  :5277-5284
  ──
  on_init → … → teardown                                          [DL, §5]
```

This is the cross-construct invariant: **register-access never resolves a region
itself** — it consumes what BRM resolved, via `map_region`, *after*
`get_resource_handle` (the ordering invariant **I06**; `body_emit.rs:5271`).
Swapping the host `(base, size)` source for a real
`pci_resource_start` / `DT_REG_ADDR` changes **no register-access operation**
(the source-swap invariant **I04**; prelude.rs:160-162). A backend must preserve this ordering and
this seam; it must **not** lower BRM ops or `map_region` in isolation. The single
load-bearing constraint the reference backend enforces today: **at most one
resolved region per driver** (`max_resolved_bindings_per_driver`,
`body_emit.rs:5072-5086`; rejected at `mod.rs:248-254`).

`register_driver` is the *other* half of the seam: DL emits the registration
marker (carrying `name` + rendered match), and BRM supplies only the **bus-path
selector token** (`bus_selector_token`, `body_emit.rs:5050-5061`), emitted right
after the DL marker (`:5203-5205`). The real per-target registration call is
backend-[Open].

---

## 7. The diag-log calls

Recognized exactly like the other call-intercepted intrinsics (§4): a
`Direct(inst)` callee whose item carries `IntrinsicKind::Log(level)`
(`body_emit.rs:2101-2104`). The reference backend lowers it to a byte-faithful
`fwrite` statement sequence rather than a C call (`emit_diag_log`,
`body_emit.rs:2722`; level tag via `level_tag`, `:6060-6063`), and the
diag-log-carrying instance is **excluded** from the ordinary
prototype/definition banks (`is_diag_log_instance`, `mod.rs:1563-1572`,
`:2204`, `:2282`) — its body is *replaced* by the lowering, not emitted as a
function.

The model carrier is **positional, not typed**: the backend reads `args[0]` as
the template byte-string operand and `args[1..]` as the flat raw value
operands. There is **no typed `LogArg` carrier in the model**, and the
reference backend explicitly never reconstructs one
(`body_emit.rs:2732-2740`). A typed `LogArg[]` shape and portable `&str`
handling remain **open design points** in the logging discussion
(`docs/discussions/logging-and-printing/logging-and-printing-discussion.md:424-429`),
and full backend-native format lowering is deferred. A new backend lowers
`Log(level) + (template, args)` to its OS logging API
(`dev_dbg`/`dev_info`/`dev_warn`/`dev_err`; `LOG_DBG`/`INF`/`WRN`/`ERR`) — see
[`model-analysis-strategy.md`](./model-analysis-strategy.md).

---

## 8. The RegseqError flow (and its boundary with native errors)

RegseqError is the **authored** semantic-error channel. As built, the reference
backend **materializes a RegseqError lowering at exactly one place: the entry
return** — the errno exit-adapter, appended only when the entry classifies as
`ErrResult` (`generic_c/mod.rs:276-294`; `emit_exit_adapter`,
`body_emit.rs:4732-4828`).

`Lifecycle::on_init`'s `Result<(), RegseqError>` is **inspected, not mapped**:
the init-fail synthesis reads the niche discriminant only to emit the
`LC on_init -> Err` trace marker and the (optional) `on_init_failure` call
(`body_emit.rs:5411-5420`, `:5464-5482`), then the process exits cleanly — no
RegseqError-to-native-result mapping, no postamble (the lifecycle surface,
`docs/discussions/regseq-device-lifecycle/device-lifecycle-surface-convergence.md:218-227`).
Mapping `on_init`'s RegseqError into a native callback result (a negative errno
returned from Linux `.probe`, a Zephyr init return code) is **future OS-backend
work** — there is no reference lowering for it.

### Recognition

1. The error enum item carries `AttrData::FrameworkType(FrameworkType::RegseqError)`
   (`domain_attr.rs:77-80`); the reference check is `is_framework_error_enum`
   (`generic_c/mod.rs:670-679`).
2. The carrier is `core::result::Result<(), RegseqError>`, a niche two-variant
   enum (Ok = variant 0, Err = variant 1). The classifier
   (`mod.rs:565-660`) requires the **compiler-certified** `Item.is_result` flag
   (`item.rs:101`) — not a rendered-name match — then checks the `()` Ok payload
   and the marked single-enum Err payload.

### Lowering (the exit-adapter boundary)

The reference lowering hand-emits an adapter `<entry>__regseq_exit`
(`emit_exit_adapter`, `body_emit.rs:4732-4828`) that:

1. calls the entry, captures the `Result` struct;
2. reads the niche discriminant (`niche_discriminant_read_block`,
   `:4654-4707` — **shared verbatim** with the DL init-fail path, `:5466`) and
   on `Ok` returns `0`;
3. otherwise switches the inner `RegseqError` tag and `return <SYMBOL>;` per the
   C02 errno table (`errno.rs:35-48`), an **unknown marked variant failing loud**
   (`EmitError::ErrnoMapping`, `mod.rs:2367`), never a silent fallback.

The errno **symbol table** and the numeric-exit form are POSIX-flavored
**OS-shell** (a Linux/Zephyr backend maps to a negative errno at probe-return,
not a process exit code); the **discriminant-read + switch-synthesis mechanism**
is reusable core. See the seam doc, § *Bank: errno adapter*.

### The authored / native boundary (a hard rule, not a backend choice)

The authored RegseqError channel is **a different error channel from native OS
resource/probe failures**, and the two must not be conflated
(`docs/discussions/regseq-error/perspective-6-authored-versus-native-error-boundary.md`):

- **Authored** errors come from code the spec author wrote returning
  `RegseqError` (e.g. inside `on_init`). These cross the lossy lowering boundary
  — the exit-adapter above.
- **Native** errors come from compiler-generated preamble operations calling OS
  APIs (`devm_ioremap_resource`, `clk_get`, `dma_request_chan`,
  `pinctrl_select_state`, …). These must propagate the **native errno/status
  directly** — *not* be round-tripped through `RegseqError`. Wrapping them
  destroys backend semantics the DSL cannot express (e.g. Linux
  `-EPROBE_DEFER`).

The boundary line is exactly the **authored-vs-generated** code split: generated
preamble returns native errors; the authored callback returns `RegseqError`. A
backend that violates this breaks probe deferral and loses platform semantics.
On `generic_c` (a host model) the native channel has no failures to propagate, so
only the authored adapter is materialized today; a real OS backend **must** keep
the two channels distinct. This is the most consequential cross-construct
constraint for a backend author, and it is revisited in
[`model-analysis-strategy.md`](./model-analysis-strategy.md) § *Native vs
authored*.

---

## 9. What the contract does **not** give you

To set expectations for a per-OS sweep, the following are **not** in the model
and must be supplied by the backend (or deferred):

- **C symbols, the entry shim, `main`/probe shape** — derived, not carried
  (§2.4; `naming.rs`).
- **The scenario set and the platform identity** — `BackendContext` inputs
  (§1.1), not model fields; the OS-framework selection a real backend derives
  from `PlatformIdentity` is itself recognized-not-lowered (§5, §1.1).
- **The errno/exit mapping** — a per-target table (`errno.rs`), not a model
  property.
- **`create` / `associate` / `teardown` / the `on_init->Err` marker** — backend-
  synthesized managed steps with no model carrier
  ([`primitive-inventory.md`](./primitive-inventory.md), rows marked
  `model_source = none`).
- **The real OS realizations** of every host hook (`pci_register_driver`,
  `devm_ioremap_resource`, `DEVICE_DT_DEFINE`, `readl`/`ioread32`, …) — all
  backend-[Open].
- **Anything for the four `recognized_not_lowered` primitives** (`poll_until`,
  `BarType::Io`, `ClassTag`, the `TargetOs` framework selection) — no reference
  lowering exists. (`UserDataField` init is covered only incidentally by the
  needs_inst-gated container zero-init `{0}`, `body_emit.rs:5301-5306` — not a
  designed per-kind lowering.)
