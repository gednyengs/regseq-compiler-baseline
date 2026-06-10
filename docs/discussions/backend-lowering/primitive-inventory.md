# Primitive Inventory — the five in-scope constructs

This is the **authoritative, citation-anchored census** of every lowering
primitive across the five in-scope constructs (register-access,
bus-resource-management, device-lifecycle, diagnostics-logging, RegseqError),
together with where each one is **defined in the model** and where (if at all)
it is **lowered by the reference backend** (`generic_c`).

It is the spine the other documents in this directory hang off of:
[`agnostic-core-vs-os-shell.md`](./agnostic-core-vs-os-shell.md) classifies the
machinery that *consumes* these primitives; [`model-analysis-strategy.md`](./model-analysis-strategy.md)
is the per-OS sweep method that walks this table row by row;
[`regseqmodel-input-contract.md`](./regseqmodel-input-contract.md) is the API
through which a backend reads the `model_source` column.

> **Citation discipline.** Every row cites a `model_source` (a
> `crates/regseq-model/...` path:line, or a surface-spec path for an authored
> taxonomy) and a `generic_c_source` (a `crates/regseq-backends/...` path:line,
> or `none`). `none` in `generic_c_source` is a load-bearing fact, not an
> omission: it means the model **recognizes** the primitive but the reference
> backend emits nothing for it. A new backend must therefore either lower it or
> fail loud — never silently drop it.

---

## Status legend

| status | meaning |
|---|---|
| **lowered** | `generic_c` emits real C for this primitive; both `model_source` and `generic_c_source` are populated and were verified as-cited. This is the reference lowering a new backend re-points at its own target. |
| **gate_lowered** | the lowering exists and is correct, but its emission gate is never satisfied by any current fixture, so no committed golden exercises it. The sibling docs call this **dormant**. Today this is exactly **`set_bus_master`** (gated on `is_pci && dma_present`; `dma_present` is a placeholder that defaults `false`). |
| **recognized_not_lowered** | the model carries/recognizes the primitive (`model_source` populated) but `generic_c` emits **nothing** for it (`generic_c_source = none`). A new backend inherits no reference here: it must design the lowering or fail loud. |
| **deferred** | the model carries the variant as explicitly-deferred sugar; `generic_c` renders only a placeholder. Today this is exactly the `MatchEntry` `Acpi`/`Usb`/`I2c`/`Spi` arms (rendered as `"?"`, `body_emit.rs:5506`). |

One **honesty qualifier** layered on top of `lowered` (see
[Coverage & scope notes](#coverage--scope-notes)): `generic_c` is a *host
model*. A `lowered` row's `generic_c_source` is the **host realization** (a
`regseq_*` trace hook over a faked register file / bus model), not a production
OS call. The real per-target realization (`pci_register_driver` /
`module_amba_driver` / `DEVICE_DT_DEFINE`, `pci_request_regions`,
`devm_ioremap_resource`, …) is **backend-[Open]** and is flagged as such
throughout, never asserted here.

---

## Inventory

| name | group | kind | status | model_source | generic_c_source |
|---|---|---|---|---|---|
| mmio_read (MmioOp::Read) | register-access | intrinsic | lowered | crates/regseq-model/src/domain_attr.rs:125-127 (op), :167 (from_name), :93 (IntrinsicKind::Mmio) | crates/regseq-backends/src/generic_c/body_emit.rs:2113-2116 (Call interception), :2297-2302 (emission); host hook crates/regseq-backends/src/generic_c/prelude.rs:87-95 |
| mmio_read_field (MmioOp::ReadField) | register-access | intrinsic | lowered | crates/regseq-model/src/domain_attr.rs:128-129, :168 | crates/regseq-backends/src/generic_c/body_emit.rs:2303-2311 |
| mmio_write (MmioOp::Write) | register-access | intrinsic | lowered | crates/regseq-model/src/domain_attr.rs:130-131, :169 | crates/regseq-backends/src/generic_c/body_emit.rs:2283-2287; host hook prelude.rs:96-102 |
| mmio_write_field (MmioOp::WriteField) | register-access | intrinsic | lowered | crates/regseq-model/src/domain_attr.rs:132-133, :170 | crates/regseq-backends/src/generic_c/body_emit.rs:2288-2296 |
| mmio_modify (MmioOp::Modify) | register-access | intrinsic | lowered | crates/regseq-model/src/domain_attr.rs:134-135, :171 | crates/regseq-backends/src/generic_c/body_emit.rs:2312-2323 |
| poll_until (regseq::poll::until) | register-access | intrinsic | recognized_not_lowered | crates/regseq-model/src/domain_attr.rs:94-95, :172; bridge recognition crates/regseq-driver/src/bridge/intrinsic/recognition.rs:90,117 | none |
| map_region | register-access | synthesized-op | lowered | crates/regseq-model/src/device_lifecycle.rs:209-229 (MmioBinding resolved base/size), :236-239 (is_resolved gate) | crates/regseq-backends/src/generic_c/body_emit.rs:5273-5284 (synthesized after get_resource_handle, traces effective BAR + resolved (base,size)); helper prelude.rs:224-232 (regseq_bus_map_region installs regseq_bus_mapped_base) |
| Mmio<T> resource + #[rseq::register_block] offset recovery | register-access | backend-mechanism | lowered | crates/regseq-model/src/device_lifecycle.rs:253-257 (ResourceKind::Mmio), :186-201 (ResourceField); Deref contract docs/discussions/register-access/register-access-surface-convergence.md:212-218 | crates/regseq-backends/src/generic_c/body_emit.rs:2439 (recover_register entry), :2461-2485 (PlaceElem::Field(idx) projection over the #[repr(C)] struct layout -> fields[idx].offset accumulation, with field-type cross-check :2479-2483), :2514-2520 (access width = size of the final register-wrapper element, validated to 1|2|4|8 bytes) |
| Field / FieldValue descriptors (mask/shift/value) | register-access | model-adt | lowered | ordinary ADTs in the model (no dedicated sibling table); closed op set crates/regseq-model/src/domain_attr.rs:125-136 (Read/ReadField/Write/WriteField/Modify only — no MmioOp::Set exists); contract register-access-surface-convergence.md:288-291 | crates/regseq-backends/src/generic_c/body_emit.rs:2282 (match over exactly the five MmioOps), :2292-2295 (.value/.mask), :2308-2310 (.mask/.shift), :2317-2322 (RMW) |
| register_driver (bus-path selector) | bus-resource-management | synthesized-op | lowered | crates/regseq-model/src/device_lifecycle.rs:55-61 (DriverDescriptor.bus_type); BusType :79-94 | crates/regseq-backends/src/generic_c/body_emit.rs:5050-5061 (selector tokens pci/platform/amba/i2c/spi/usb/Other), :5140-5154 + :5203-5205 (emission); helper prelude.rs:188-190 (regseq_bus_register_driver, traces selector + binding=bound|none) |
| enable_device | bus-resource-management | synthesized-op | lowered | gate: crates/regseq-model/src/device_lifecycle.rs:96-104 (BusType::is_pci_like); synthesized-not-authored per docs/discussions/bus-resource-management/bus-resource-management-surface-convergence.md:87,128 | crates/regseq-backends/src/generic_c/body_emit.rs:5256-5259 (PCI-gated: emitted iff is_pci, inside the resolved-binding block); helper prelude.rs:198-201 (records regseq_bus_enabled) |
| claim_regions | bus-resource-management | synthesized-op | lowered | per region-bearing MmioBinding, crates/regseq-model/src/device_lifecycle.rs:209-229; gating matrix bus-resource-management-surface-convergence.md:129,133-135 | crates/regseq-backends/src/generic_c/body_emit.rs:5260-5266 (per resolved binding, on EVERY bus — not PCI-gated); helper prelude.rs:209-211 (trace-only no-op carrying effective BAR) |
| set_bus_master | bus-resource-management | synthesized-op | gate_lowered | crates/regseq-model/src/device_lifecycle.rs:62-70 (DriverDescriptor.dma_present — DMA-Management surface-presence placeholder bit, defaults false; no driver sets it today, so set_bus_master is not emitted yet; wiring it is the DMA construct's job, GRILL O2) | crates/regseq-backends/src/generic_c/body_emit.rs:5267-5270 (emitted ONLY when is_pci && driver.descriptor.dma_present, gate at :5268); helper prelude.rs:212-219 (comment :212-215: 'Defaults absent today, so this is emitted by no fixture yet'; records regseq_bus_mastered :216-218) |
| get_resource_handle | bus-resource-management | synthesized-op | lowered | synthesized, not authored (R04) — bus-resource-management-surface-convergence.md:87,131 (unconditional, captures the bus-handle context, I06) | crates/regseq-backends/src/generic_c/body_emit.rs:5271-5272; helper prelude.rs:220-223 (regseq_bus_get_handle) |
| read_config (ConfigOp::Read) | bus-resource-management | intrinsic | lowered | crates/regseq-model/src/domain_attr.rs:110-113 (+ family :96-100), :175 (from_name) | crates/regseq-backends/src/generic_c/body_emit.rs:2122-2125 (interception), :2364-2375 (emission), :2404-2414 (width validation); host hooks prelude.rs:260-288 (incl. PCI-ID-caching rule :272-288) |
| write_config (ConfigOp::Write) | bus-resource-management | intrinsic | lowered | crates/regseq-model/src/domain_attr.rs:114-117, :176 | crates/regseq-backends/src/generic_c/body_emit.rs:2376-2394; host hook prelude.rs:289-295 |
| MmioBinding (L1 id/bar/type + resolved L2 base/size, effective_bar = L2.bar ?? L1.bar) | bus-resource-management | model-struct | lowered | crates/regseq-model/src/device_lifecycle.rs:203-229 (binding), :236-239 (is_resolved); ResourceField.binding :192-200 | crates/regseq-backends/src/generic_c/body_emit.rs:5244-5284 (consumption), :5006-5012 (model_has_bus_binding gate), :5020-5026 (model_has_bus_type gate), :5037-5043 (config-intrinsic gate) |
| BarType::Mem (memory-mapped carrier, default) | bus-resource-management | model-enum | lowered | crates/regseq-model/src/device_lifecycle.rs:244-247 (enum :244; #[default] Mem :246-247), paired with BarType::Io :248-249 | the default carrier of every lowered MMIO region — consumed through the resolved MmioBinding -> map_region path, crates/regseq-backends/src/generic_c/body_emit.rs:5273-5284 |
| BarType::Io (port-I/O carrier, type = "io") | bus-resource-management | model-enum | recognized_not_lowered | crates/regseq-model/src/device_lifecycle.rs:248-249 (variant; PCI-like bus only, GRILL C01/BRM-003), enum :244 | none |
| BusType taxonomy (Pci / Platform / Amba lowered; I2c / Spi / Usb / Other deferred) | bus-resource-management | model-enum | lowered | crates/regseq-model/src/device_lifecycle.rs:79-94 (I2c/Spi/Usb annotated 'deferred lowering'), :101-103 (is_pci_like: only Pci) | crates/regseq-backends/src/generic_c/body_emit.rs:5050-5061 (all tokens render in the registration trace; only Pci unlocks the PCI-gated ops) |
| TargetOs::Linux / TargetOs::Zephyr (closed OS target vocabulary) | cross-construct | model-enum | recognized_not_lowered | crates/regseq-model/src/target.rs:69-74 (closed enum; Linux :71, Zephyr :73), :91-97 (TargetOs::from_token, case-insensitive closed-vocabulary parse), :114-116 (carried as PlatformIdentity.os) | carried but not acted on: BackendContext threads PlatformIdentity (crates/regseq-backends/src/generic_c/backend_options.rs:131, :146); generic_c only traces the os token as a C header comment (mod.rs:208-211, platform_trace_comment) — no framework selection (target.rs:108-109: 'generic_c ignores it') |
| driver registration (LC register: descriptor name + typed match set) | device-lifecycle | synthesized-op | lowered | crates/regseq-model/src/device_lifecycle.rs:47-52 (DriverDescriptor.name, match_set) | crates/regseq-backends/src/generic_c/body_emit.rs:5196-5199; trace hook prelude.rs:120-122; match rendering body_emit.rs:5499-5509 |
| MatchEntry: Devicetree / Pci / Amba | device-lifecycle | model-enum | lowered | crates/regseq-model/src/device_lifecycle.rs:110-121 (Devicetree :112, Pci :114-119, Amba :121) | crates/regseq-backends/src/generic_c/body_emit.rs:5501-5505 (dt:<compatible> / pci:VVVV:DDDD / amba:id/mask trace forms) |
| MatchEntry: Acpi / Usb / I2c / Spi | device-lifecycle | model-enum | deferred | crates/regseq-model/src/device_lifecycle.rs:122-129 (sugar marked deferred) | crates/regseq-backends/src/generic_c/body_emit.rs:5506 (renders "?") |
| device_class (ClassTag) | device-lifecycle | model-enum | recognized_not_lowered | crates/regseq-model/src/device_lifecycle.rs:53-54, :134-150 (Generic/Serial/Network/Other; 'stored, unvalidated this stage') | none |
| create (LC create marker + gated instance allocation) | device-lifecycle | synthesized-op | lowered | crates/regseq-model/src/device_lifecycle.rs:176-183 (DriverHandle) | two distinct emissions: (a) the trace marker regseq_lc_create() is ALWAYS emitted, crates/regseq-backends/src/generic_c/body_emit.rs:5206 (trace hook prelude.rs:123-125); (b) the zero-initialized container instance `struct <target> inst = {0}` is SEPARATELY gated by needs_inst — declared iff a callback invoked in the selected scenario is overridden (avoids -Wunused for an empty impl) — gate :5287-5300, emission :5301-5306 |
| associate (instance <-> device association) | device-lifecycle | synthesized-op | lowered | part of the managed bring-up state machine (device-lifecycle construct); no dedicated model field | crates/regseq-backends/src/generic_c/body_emit.rs:5207; trace hook prelude.rs:126-128 |
| resource preamble (per resource field, forward declaration order) | device-lifecycle | synthesized-op | lowered | crates/regseq-model/src/device_lifecycle.rs:178-182 (resource_fields, declaration order preserved :186-187) | crates/regseq-backends/src/generic_c/body_emit.rs:5209-5217 (runs for every scenario, before the bus preamble and on_init); trace hook prelude.rs:138-140 |
| resource postamble (reverse order; skipped(surprise) when device gone) | device-lifecycle | synthesized-op | lowered | crates/regseq-model/src/device_lifecycle.rs:178-182 | crates/regseq-backends/src/generic_c/body_emit.rs:5391-5406 (emit_teardown_dispatch, reverse iteration + skipped flag); trace hook prelude.rs:141-147 |
| teardown dispatch (present=1|0) | device-lifecycle | synthesized-op | lowered | scenario semantics (orderly vs surprise), device-lifecycle construct | crates/regseq-backends/src/generic_c/body_emit.rs:5398 (LC teardown present=N), shared by orderly/surprise via emit_teardown_dispatch :5391-5406; trace hook prelude.rs:132-134 |
| on_init callback | device-lifecycle | callback | lowered | crates/regseq-model/src/device_lifecycle.rs:162-163 (LifecycleOps.on_init); override semantics :152-158 | crates/regseq-backends/src/generic_c/body_emit.rs:5308-5311 (marker always emitted, CR-3; author call iff overridden), Result handling per scenario :5313-5366 |
| on_deinit callback | device-lifecycle | callback | lowered | crates/regseq-model/src/device_lifecycle.rs:164-165 | crates/regseq-backends/src/generic_c/body_emit.rs:5315-5322 (orderly: teardown dispatch present=1 + always-on marker) |
| on_surprise_remove callback | device-lifecycle | callback | lowered | crates/regseq-model/src/device_lifecycle.rs:166 | crates/regseq-backends/src/generic_c/body_emit.rs:5323-5339 (surprise: present=0 + always-on marker; postambles skipped) |
| on_init_failure callback | device-lifecycle | callback | lowered | crates/regseq-model/src/device_lifecycle.rs:167-168 | crates/regseq-backends/src/generic_c/body_emit.rs:5340-5354 + emit_init_fail_body :5409-5479 (capture on_init's niche-encoded Result, branch on Err discriminant, emit LC on_init -> Err + always-on marker + author body iff overridden) |
| on_system_shutdown callback | device-lifecycle | callback | lowered | crates/regseq-model/src/device_lifecycle.rs:169-170 ('slot omitted when None') | crates/regseq-backends/src/generic_c/body_emit.rs:5355-5366 (marker + call ONLY when overridden — the lone omit-when-absent callback, D10/I02) |
| scenario: orderly | device-lifecycle | scenario | lowered | none — harness-side concept (RD-5); defined crates/regseq-backends/src/generic_c/backend_options.rs:25-26, tokens :44-52 | crates/regseq-backends/src/generic_c/body_emit.rs:5315-5322 |
| scenario: surprise | device-lifecycle | scenario | lowered | none — harness-side (backend_options.rs:27-28) | crates/regseq-backends/src/generic_c/body_emit.rs:5323-5339 |
| scenario: init-fail | device-lifecycle | scenario | lowered | none — harness-side (backend_options.rs:29-30) | crates/regseq-backends/src/generic_c/body_emit.rs:5340-5354, :5409-5479 |
| scenario: shutdown | device-lifecycle | scenario | lowered | none — harness-side (backend_options.rs:31-32) | crates/regseq-backends/src/generic_c/body_emit.rs:5355-5366 |
| target-view container morph (Policy-P: void* __rseq_dev + inner + uint32_t __rseq_state) | device-lifecycle | backend-mechanism | lowered | crates/regseq-model/src/device_lifecycle.rs:36-43 (DriverLifecycle), :176-183 (DriverHandle) | crates/regseq-backends/src/generic_c/body_emit.rs:5174-5189 (struct emission); the zero-init instance `inst = {0}` is needs_inst-gated (:5287-5300 gate, :5301-5306 emission); callbacks run spec-view over &inst.inner, CR-7 |
| user-data fields (Rule-1 ZeroInit / Rule-2 Option<T: Copy>) | device-lifecycle | model-struct | lowered | crates/regseq-model/src/device_lifecycle.rs:262-279 (UserDataField, UserDataKind) | crates/regseq-backends/src/generic_c/body_emit.rs:5305 (covered by the blanket zero-initialized container `inst = {0}` — itself needs_inst-gated, :5287-5300) |
| lifecycle runner (__rseq_lc_run) + scenario helper functions | device-lifecycle | backend-mechanism | lowered | crates/regseq-model/src/device_lifecycle.rs:27-31 (DeviceLifecycleTables.drivers, BTreeMap -> deterministic order) | crates/regseq-backends/src/generic_c/body_emit.rs:4975-4979 (symbol), :5088-5096 (emit_lifecycle_synthesis, empty for no-lifecycle TU), :5191-5193 (one static __rseq_lc_<idx>_<label>_<scenario> per selected scenario), :5376 (runner emission) |
| log-debug (LogLevel::Debug) | diagnostics-logging | intrinsic | lowered | crates/regseq-model/src/domain_attr.rs:141-143, :161; family :90 | crates/regseq-backends/src/generic_c/body_emit.rs:2100-2104 (interception), :2722-2849 (emit_diag_log); tag '[DEBUG] ' via level_tag :6058-6064 |
| log-info (LogLevel::Info) | diagnostics-logging | intrinsic | lowered | crates/regseq-model/src/domain_attr.rs:144-145, :162 | crates/regseq-backends/src/generic_c/body_emit.rs:2100-2104, :2722-2849; tag 'INFO' :6062 |
| log-warn (LogLevel::Warn) | diagnostics-logging | intrinsic | lowered | crates/regseq-model/src/domain_attr.rs:146-147, :163 | crates/regseq-backends/src/generic_c/body_emit.rs:2100-2104, :2722-2849; tag 'WARN' :6061 |
| log-error (LogLevel::Error) | diagnostics-logging | intrinsic | lowered | crates/regseq-model/src/domain_attr.rs:148-149, :164 | crates/regseq-backends/src/generic_c/body_emit.rs:2100-2104, :2722-2849; tag 'ERROR' :6060 |
| format template + placeholder modifiers (default {} + 8 hex forms) | diagnostics-logging | backend-mechanism | lowered | template literal carried as a const-aggregate &str carrier in the model (D6); Pipeline-A flat args [fmt, v1..vN] per body_emit.rs:2712-2715 | crates/regseq-backends/src/generic_c/body_emit.rs:5798-5850 (diag_specifier_and_cast_for), :5871-5884 (Modifier shape: None + Hex{upper,alternate,width}), :5956-5999 (parse_placeholder — the eight accepted hex forms enumerated by exact byte match at :5966+: {x,X} x {plain, #, 08, #010}), :5903-5950 (parse_template: {{/}} escapes, fail-loud on malformed braces), :2741-2750 (arity check) |
| &str log arguments (static literal + non-static fat pointer) | diagnostics-logging | backend-mechanism | lowered | &str carrier = Aggregate([Reloc->CMS, Uint(len)]) read per body_emit.rs:2876-2918; rust_str fat pointer crates/regseq-backends/src/generic_c/prelude.rs:41-44 | crates/regseq-backends/src/generic_c/body_emit.rs:2794-2802 (static: fwrite of decoded bytes with explicit count), :2812-2817 (non-static: fwrite((op).data, 1, (op).len)); hex modifier on &str rejected :5856-5864 |
| RegseqError framework-type marker | regseq-error | model-enum | lowered | crates/regseq-model/src/domain_attr.rs:62-68 (AttrData::FrameworkType, discriminant 1 at :68), :75-80 (FrameworkType::RegseqError = 0, enum :77) | crates/regseq-backends/src/generic_c/mod.rs:670-679 (is_framework_error_enum), :565-660 (classify_entry_return: is_result certification + Ok/Err name guard + unit Ok payload + marked Err enum) |
| RegseqError::InvalidParameter -> EINVAL | regseq-error | errno-mapping | lowered | variant catalog pinned at docs/discussions/regseq-error/regseq-error-spec.md:221-229; carried as VariantLayout.name on the marked enum | crates/regseq-backends/src/generic_c/errno.rs:37; case emission body_emit.rs:4798-4809 |
| RegseqError::OutOfMemory -> ENOMEM | regseq-error | errno-mapping | lowered | docs/discussions/regseq-error/regseq-error-spec.md:222 | crates/regseq-backends/src/generic_c/errno.rs:38 |
| RegseqError::HardwareError -> EIO | regseq-error | errno-mapping | lowered | docs/discussions/regseq-error/regseq-error-spec.md:221-229 | crates/regseq-backends/src/generic_c/errno.rs:39 |
| RegseqError::Timeout -> ETIMEDOUT | regseq-error | errno-mapping | lowered | docs/discussions/regseq-error/regseq-error-spec.md:221-229 | crates/regseq-backends/src/generic_c/errno.rs:40 |
| RegseqError::Unsupported -> EOPNOTSUPP | regseq-error | errno-mapping | lowered | docs/discussions/regseq-error/regseq-error-spec.md:221-229 | crates/regseq-backends/src/generic_c/errno.rs:41 |
| RegseqError::DeviceBusy -> EBUSY | regseq-error | errno-mapping | lowered | docs/discussions/regseq-error/regseq-error-spec.md:221-229 | crates/regseq-backends/src/generic_c/errno.rs:42 |
| RegseqError::PermissionDenied -> EPERM | regseq-error | errno-mapping | lowered | docs/discussions/regseq-error/regseq-error-spec.md:221-229 | crates/regseq-backends/src/generic_c/errno.rs:43 |
| RegseqError::DeviceNotFound -> ENODEV | regseq-error | errno-mapping | lowered | docs/discussions/regseq-error/regseq-error-spec.md:221-229 | crates/regseq-backends/src/generic_c/errno.rs:44 |
| RegseqError::GenericFailure -> EIO | regseq-error | errno-mapping | lowered | docs/discussions/regseq-error/regseq-error-spec.md:229 | crates/regseq-backends/src/generic_c/errno.rs:45 |
| Ok(()) -> exit 0 success mapping | regseq-error | errno-mapping | lowered | Result<(), RegseqError> niche shape certified at crates/regseq-backends/src/generic_c/mod.rs:565-660 (Ok payload must be exactly (), :627-635) | crates/regseq-backends/src/generic_c/body_emit.rs:4819-4821 (return 0 on the Ok niche discriminant, R04) |
| errno exit-adapter (<entry>__regseq_exit) | regseq-error | backend-mechanism | lowered | AdapterSpec minted at crates/regseq-backends/src/generic_c/mod.rs:521-534; only the DEFAULT entry suffix may return an error this phase (:514-519) | crates/regseq-backends/src/generic_c/body_emit.rs:4709-4828 (emit_exit_adapter: niche-discriminant read, Ok->0, tag-view switch of RegseqError); errno.rs:35-48 (name-keyed C02 table); demand-driven #include <errno.h> prelude.rs:345-350, :434-439 |
| entry-shim classification (UnitVoid / ErrResult / LifecycleOnly) | cross-construct | backend-mechanism | lowered | entry resolution over instances by emission_name suffix, crates/regseq-backends/src/generic_c/mod.rs:432-540 (DEFAULT_ENTRY_SUFFIX 'regseq_main' :50); return classification :565-660 | crates/regseq-backends/src/generic_c/naming.rs:73-103 (EntryShim), :131+ (generate_main_c); lifecycle-runner composition mod.rs:436-460; B3 early-errno-return in the ErrResult+runner main: naming.rs:181-182 (`int rc = adapter(); if (rc != 0) return rc;` before invoking the runner) |

---

## Coverage & scope notes

**Revision record (2026-06-09, applying the Codex review; all citations
re-verified against the working tree, worktree `backend-spec`).** Relative to
the prior census this revision **added 2 rows**: (1) `TargetOs::Linux/Zephyr` —
the closed OS vocabulary at `target.rs:69-74` (variants `:71`/`:73`),
`from_token :91-97`, `PlatformIdentity.os :114-116`; status
`recognized_not_lowered` because `generic_c` carries `PlatformIdentity` through
`BackendContext` (`backend_options.rs:131,:146`) and traces the os token as a
header comment (`mod.rs:208-211`) but performs no framework selection
(`target.rs:108-109`). (2) `BarType::Mem` — the `#[default]` MMIO carrier at
`device_lifecycle.rs:244-247`, paired with `Io :248-249`. **Corrected 3
mis-citations:** `Mmio<T>` offset recovery now cites the `Field(idx)`
accumulation `:2461-2485` and width recovery `:2514-2520` (not just the
`recover_register` entry `:2439`); the eight hex placeholder forms are
enumerated in `parse_placeholder` at `body_emit.rs:5966+` (`Modifier`
`:5871-5884` only declares the shape); the B3 early errno return is
`naming.rs:181-182`, not `:84`. **Applied 3 status/content fixes:**
`set_bus_master` relabelled `gate_lowered` (emitted only under
`is_pci && dma_present` at `body_emit.rs:5268`; `dma_present` defaults false per
`device_lifecycle.rs:62-70`; `prelude.rs:212-215` confirms no fixture emits it
today); the `create` row un-conflates the always-emitted `regseq_lc_create()`
marker (`:5206`) from the `needs_inst`-gated zero-init instance (gate
`:5287-5300`, emission `:5301-5306`), with the same gate noted on the
container-morph and user-data rows; the `Field`/`FieldValue` row drops the stale
`register-access-surface-convergence.md:243` citation (superseded `set(...)`
text) in favor of the closed five-op `MmioOp` set at `domain_attr.rs:125-136`
and the exhaustive five-op match at `body_emit.rs:2282` — no `MmioOp::Set`
exists. Coverage anchors confirmed in-tree: `FrameworkType` enum
`domain_attr.rs:77`, `IntrinsicKind :88`, `ConfigOp :110`, `MmioOp :125`,
`LogLevel :141`, `LifecycleOps` override semantics `device_lifecycle.rs:152-158`.

**Scope.** Exactly the five in-scope constructs are covered, plus the
cross-construct seams (bus preamble ordered inside lifecycle synthesis;
`map_region` consuming the BRM-resolved `(base,size)`; entry-shim/errno-adapter
composition). Nothing here applies to **interrupts, DMA, or serial/data-path** —
those are out of scope. Where an out-of-scope construct's *fact* leaks in (the
`dma_present` bit on `DriverDescriptor`), it appears only as a gate input, never
as a lowered primitive (`dma_present` is explicitly the DMA construct's future
fact, GRILL O2).

**The non-lowered surfaces, flagged honestly rather than overclaimed:**

- **`set_bus_master` — `gate_lowered` (dormant).** The lowering is implemented
  (`body_emit.rs:5267-5270`) but gated on `is_pci && dma_present`;
  `dma_present` is a placeholder that defaults `false`
  (`device_lifecycle.rs:62-70`), so the op is emitted by **no fixture today**.
- **The four `recognized_not_lowered` rows** — `poll_until`, `BarType::Io`,
  `device_class` (`ClassTag`), and the `TargetOs` framework selection — are
  carried in the model but have no lowering act on them. A new backend inherits
  no reference lowering for any of them and must design one or fail loud.
- **The `deferred` row** — the `MatchEntry` `Acpi`/`Usb`/`I2c`/`Spi` sugar
  (`device_lifecycle.rs:122-129`) renders only as `"?"`
  (`body_emit.rs:5506`).
- **`UserDataField`/`UserDataKind` is `lowered` only in the blanket sense:** the
  needs_inst-gated container zero-init `{0}` (`body_emit.rs:5301-5306`)
  satisfies Rule-1/Rule-2 init implicitly. That is incidental coverage, not a
  designed per-kind lowering of the classification.

**`generic_c` is a host model; the real OS realizations are backend-[Open].**
Every `lowered` row's host hook documents its intended per-target realization in
the prelude comments but does **not** implement it:

- `register_driver` → `pci_register_driver` / `module_amba_driver` /
  `DEVICE_DT_DEFINE` (prelude.rs:186-190).
- `claim_regions` → Linux PCI `pci_request_selected_regions`; Linux AMBA fuses
  claim∘map into one `devm_ioremap_resource`; Zephyr has no runtime claim
  (prelude.rs:202-211).
- `map_region`'s `(base, size)` source → `pci_resource_start` / `DT_REG_ADDR`
  in place of the host `[mmio.<id>]` literal, **changing no register-access
  operation** (invariant I04, prelude.rs:160-162).

**OS-target version specifics remain OPEN QUESTIONS.** Linux 6.6+ PCI vs
platform/DT accessor selection (`readl`/`writel` vs `ioread32`/`iowrite32`;
`pci_iomap` vs `devm_ioremap_resource`), Zephyr 3.7/4.x `DEVICE_DT_DEFINE` /
`DEVICE_MMIO_*`, and Zephyr-over-PCI are all deferred to the per-OS sweep
([`model-analysis-strategy.md`](./model-analysis-strategy.md)) and are not
asserted here.

**Cross-construct composition is preserved in the citations** (and is the
subject of [`agnostic-core-vs-os-shell.md`](./agnostic-core-vs-os-shell.md) §
*The cross-construct seam*): `map_region` (register-access) is sourced **inside**
the device-lifecycle preamble *after* the BRM ops (`body_emit.rs:5244-5284`);
`register_driver` shows BRM supplying only the bus-path selector while DL owns
the registration marker; the `Result` niche-discriminant read is **shared**
between the RegseqError exit-adapter and the DL init-fail path
(`niche_discriminant_read_block`, `body_emit.rs:4654-4707`); and the authored
RegseqError channel is noted as **distinct** from native OS resource/probe
failures (see [`regseqmodel-input-contract.md`](./regseqmodel-input-contract.md)
§ *The RegseqError flow*).
