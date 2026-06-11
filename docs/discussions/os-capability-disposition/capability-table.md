# Ratified per-OS capability table — human ratification record

This is the **doc mirror** of the typed source of truth in
`crates/regseq-backends/src/capability.rs` (GRILL `os-capability-disposition`
D03). The two are bound by the `capability_table_matches_ratified_doc` unit
test: every code row's (ID, key fields, disposition, pin, citation) must
appear here verbatim, and no row may exist here without a code row. Edit the
Rust table first; this record follows at ratification time.

Row key (D02): `(target os, os version pin, bus, registration form, slot)` —
the registration form carries the impossibility claim; a `bus` of `any`
wildcards the queried bus (incl. PCIe-over-DT). Row IDs follow the C02
grammar `CAP-<OS>-<FORM>-<SLOT>-<N>`, are append-only, and are never renamed;
a re-ratification that changes substance mints `N+1`.

Dispositions (amendment 1 as amended, D01; appended by the device-interrupt
unit per its GRILL D03/C02): `Delivered`, `AbsentDispatch` (the ONLY
disposition that may non-emit an authored CALLBACK), `DesignedScheduled`
(reject-at-emit with the scheduling diagnostic), `NeedsNewSurface`,
`RejectedUnsupported`, and `StructurallyAbsent` (a compiler-managed
OPERATION — not an authored callback — structurally absent on the
registration form; renders its own `RSEQ-ABSENT-OP` audit-block kind, never
the `RSEQ-ABSENT-SLOT` spelling; first occupant:
`interrupt.register_handler.deregister` on the static-connect form).

## Rows (as ratified 2026-06-10; anchors verified per the D05 bar)

| row id | target os | pin | bus | registration | slot | disposition | citation |
|---|---|---|---|---|---|---|---|
| CAP-ZEPHYR-DT-SURPRISE-1 | zephyr | v4.4.0 (684c9e8f32e4) | any | static-devicetree | on_surprise_remove | AbsentDispatch | Zephyr v4.4.0 (684c9e8f32e4) static DT device model, read live in the eval-workspace checkout 2026-06-10 (os-capability-disposition T02): DEVICE_DT_INST_DEFINE (include/zephyr/device.h:283) expands through Z_DEVICE_DEFINE (device.h:1317) to Z_DEVICE_BASE_DEFINE (device.h:1255), a const struct device placed in a link-time iterable section (STRUCT_SECTION_ITERABLE_NAMED_ALTERNATE, device.h:1259) — instances are build-time-fixed; the runtime device API carries init/deinit only (device_init device.h:906, device_deinit device.h:928) and device.h names no hot-remove/unplug dispatch API (named-API absence). No dispatch path exists for surprise removal (B-7; A-12) |
| CAP-ZEPHYR-DT-SHUTDOWN-1 | zephyr | v4.4.0 (684c9e8f32e4) | any | static-devicetree | on_system_shutdown | DesignedScheduled | PM-device slot witnessed, wrapper designed (scaffold §4); trigger contract pending C-4 pin, per A-18 |
| CAP-ZEPHYR-DT-DEINIT-1 | zephyr | v4.4.0 (684c9e8f32e4) | any | static-devicetree | on_deinit | Delivered | `device_deinit()` device.h:928; `DEVICE_DT_DEINIT_DEFINE` device.h:243; rc returned verbatim kernel/device.c:199; `CONFIG_DEVICE_DEINIT_SUPPORT` kernel/Kconfig.device:36; delivered by spec-convergence-pl011 GRILL D03 |
| CAP-LINUX-PCI-SURPRISE-1 | linux | v6.6 | pci | pci-dynamic | on_surprise_remove | Delivered | present-predicate dispatch (pci_device_is_present) in `.remove`, e1000-composed/golden.linux.c:370; v6.6 is the linux-backend docs' verification pin (linux-open-questions.md, git.kernel.org ?h=v6.6 anchors) |
| CAP-ZEPHYR-STATICIRQ-DEREGISTER-1 | zephyr | v4.4.0 (684c9e8f32e4) | any | static-devicetree | interrupt.register_handler.deregister | StructurallyAbsent | Zephyr v4.4.0 (684c9e8f32e4) static IRQ_CONNECT, re-verified live in the eval-workspace checkout 2026-06-11 (device-interrupt T10): IRQ_CONNECT (include/zephyr/irq.h:48) expands to ARCH_IRQ_CONNECT, a BUILD-TIME sw_isr_table install; the only disconnect API is irq_disconnect_dynamic (irq.h:90), documented for irq_connect_dynamic clients (z_isr_uninstall sw_isr_table.h:302 is its dynamic-table internal) — no runtime disconnect exists for a static connect. irq_disable IS emitted at deinit; the line stays connected-but-masked (GRILL device-interrupt D03/C02) |

NEVER_ABSENT: on_init, on_init_failure

The `NEVER_ABSENT` slots may never pair with `AbsentDispatch` on ANY OS
(D14: "probe cannot fail" is almost certainly false once allocation, mapping
and registration are included) — enforced by
`never_absent_slots_have_no_absent_rows`.

## Re-pin protocol (amendment 9, D12)

`ZEPHYR_PIN` in `capability.rs` is the single authoritative current pin. On
a re-pin, `absent_dispatch_rows_match_current_os_pin` fails naming each
Zephyr `AbsentDispatch`/`DesignedScheduled`/`StructurallyAbsent` row still
carrying the old pin (the structurally-absent impossibility claim is just as
version-bound — device-interrupt unit);
re-review = re-verify the claim on the new pin, update the row's
pin+citation in the Rust table, and mirror it here (forced by the sync
test).
