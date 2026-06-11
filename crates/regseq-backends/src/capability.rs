//! The ratified per-OS capability table (os-capability-disposition unit).
//!
//! Amends the D.2 fail-loud discipline with exactly one new mechanism: an
//! authored lifecycle callback whose dispatch condition structurally cannot
//! occur on a target OS may be *auditably non-emitted* — named audit block
//! in the generated C, conformance-report row, test-log line — instead of
//! rejected, with the body still fully checked (GRILL
//! `os-capability-disposition`, amendments 1–9 as ratified 2026-06-10;
//! founding doc `docs/discussions/os-capability-disposition/`).
//!
//! Source of truth = this typed static table (D03); the human ratification
//! record is the markdown mirror under
//! `docs/discussions/os-capability-disposition/capability-table.md`, kept
//! honest by a sync test. Emitters may reference rows ONLY via the named
//! consts below (amendment 2, structural: free-form claims are
//! unrepresentable). Everything not in a ratified `AbsentDispatch` row keeps
//! fail-loud rejection.

use regseq_model::TargetOs;

/// The closed disposition enum (amendment 1 as amended, D01).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Delivered,
    AbsentDispatch,    // ONLY this variant may non-emit an authored callback
    DesignedScheduled, // slot witnessed/designed; emission scheduled behind a
    // named pin or successor; reject-at-emit with the
    // scheduling diagnostic (never the word "unsupported")
    NeedsNewSurface,     // model/fixture gap; escalate, blocks
    RejectedUnsupported, // fail-loud D.2 default
}

/// The registration form carrying the impossibility claim (D02: the zephyr
/// E1000 is PCIe but registered via static DT — the form, not the bus,
/// decides dispatch reachability).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RegistrationForm {
    StaticDevicetree, // Zephyr DEVICE_DT_INST_DEFINE family
    PciDynamic,       // Linux PCI probe/remove registration
}

/// Backend-side slot names for `regseq_model::LifecycleOps` fields.
/// (Backend concern: defined HERE, not in regseq-model.)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LifecycleSlot {
    OnInit,
    OnInitFailure,
    OnDeinit,
    OnSurpriseRemove,
    OnSystemShutdown,
}

impl LifecycleSlot {
    /// The model-side slot name (the `LifecycleOps` field), e.g.
    /// `"on_surprise_remove"` — the spelling every artifact renders.
    pub fn model_name(self) -> &'static str {
        match self {
            LifecycleSlot::OnInit => "on_init",
            LifecycleSlot::OnInitFailure => "on_init_failure",
            LifecycleSlot::OnDeinit => "on_deinit",
            LifecycleSlot::OnSurpriseRemove => "on_surprise_remove",
            LifecycleSlot::OnSystemShutdown => "on_system_shutdown",
        }
    }
}

/// One ratified capability-table row (C01).
#[derive(Debug)]
pub struct CapabilityRow {
    /// Stable ratified ID per the C02 grammar `CAP-<OS>-<FORM>-<SLOT>-<N>`.
    /// Append-only; never renamed; re-ratification that changes substance
    /// mints N+1.
    pub id: &'static str,
    pub target_os: TargetOs,
    /// Pinned OS version incl. commit, e.g. "v4.4.0 (684c9e8f32e4)" (D05).
    pub os_version_pin: &'static str,
    pub bus_family: &'static str,
    pub registration_form: RegistrationForm,
    pub slot: LifecycleSlot,
    pub disposition: Disposition,
    /// Non-empty; must contain the pin + >=1 source anchor in the pinned
    /// local checkout or vendored reference (D05; L3 empty-ratified rule).
    pub citation: &'static str,
}

impl CapabilityRow {
    /// The C04 `RSEQ-ABSENT-SLOT` audit block for this row, grammar
    /// verbatim — generated from the same const as the report row, so
    /// cross-artifact row-ID consistency holds by construction (D10).
    pub fn audit_block(&self) -> String {
        format!(
            "/* RSEQ-ABSENT-SLOT: {}\n * disposition: AbsentDispatch row: {}\n * [ratified: {}]\n * body checked, not delivered: dispatch cannot occur on this target. */\n",
            self.slot.model_name(),
            self.id,
            self.citation
        )
    }
}

/// Single authoritative current pin (D12) — the one place a re-pin edits.
pub const ZEPHYR_PIN: &str = "v4.4.0 (684c9e8f32e4)";

/// Slots that may NEVER carry Disposition::AbsentDispatch, on any OS (D14:
/// "probe cannot fail" is almost certainly false once allocation/mapping/
/// registration are included).
pub const NEVER_ABSENT: &[LifecycleSlot] = &[LifecycleSlot::OnInit, LifecycleSlot::OnInitFailure];

/// Zephyr static-DT `on_surprise_remove`: the one ratified absent slot
/// (C03 row 1; resolves B-7 — answer: not representable, ratified).
pub const CAP_ZEPHYR_DT_SURPRISE_1: CapabilityRow = CapabilityRow {
    id: "CAP-ZEPHYR-DT-SURPRISE-1",
    target_os: TargetOs::Zephyr,
    os_version_pin: ZEPHYR_PIN,
    bus_family: "any", // incl. PCIe-over-DT (D02: the FORM carries the claim)
    registration_form: RegistrationForm::StaticDevicetree,
    slot: LifecycleSlot::OnSurpriseRemove,
    disposition: Disposition::AbsentDispatch,
    citation: "Zephyr v4.4.0 (684c9e8f32e4) static DT device model, read live in the \
               eval-workspace checkout 2026-06-10 (os-capability-disposition T02): \
               DEVICE_DT_INST_DEFINE (include/zephyr/device.h:283) expands through \
               Z_DEVICE_DEFINE (device.h:1317) to Z_DEVICE_BASE_DEFINE (device.h:1255), a \
               const struct device placed in a link-time iterable section \
               (STRUCT_SECTION_ITERABLE_NAMED_ALTERNATE, device.h:1259) — instances are \
               build-time-fixed; the runtime device API carries init/deinit only \
               (device_init device.h:906, device_deinit device.h:928) and device.h names no \
               hot-remove/unplug dispatch API (named-API absence). No dispatch path exists \
               for surprise removal (B-7; A-12)",
};

/// Zephyr `on_system_shutdown`: witnessed/designed, scheduled (C03 row 2).
pub const CAP_ZEPHYR_DT_SHUTDOWN_1: CapabilityRow = CapabilityRow {
    id: "CAP-ZEPHYR-DT-SHUTDOWN-1",
    target_os: TargetOs::Zephyr,
    os_version_pin: ZEPHYR_PIN,
    bus_family: "any",
    registration_form: RegistrationForm::StaticDevicetree,
    slot: LifecycleSlot::OnSystemShutdown,
    disposition: Disposition::DesignedScheduled,
    citation: "PM-device slot witnessed, wrapper designed (scaffold §4); trigger contract \
               pending C-4 pin, per A-18",
};

/// Zephyr `on_deinit`: exists v4.2.0+, unverified/undesigned, scheduled
/// (C03 row 3).
pub const CAP_ZEPHYR_DT_DEINIT_1: CapabilityRow = CapabilityRow {
    id: "CAP-ZEPHYR-DT-DEINIT-1",
    target_os: TargetOs::Zephyr,
    os_version_pin: ZEPHYR_PIN,
    bus_family: "any",
    registration_form: RegistrationForm::StaticDevicetree,
    slot: LifecycleSlot::OnDeinit,
    disposition: Disposition::DesignedScheduled,
    citation: "`device_deinit()` exists v4.2.0+; unverified/undesigned (B-6, C-3, D06); \
               delivered by spec-convergence-pl011",
};

/// Linux PCI `on_surprise_remove`: delivered (C03 row 4 — the
/// present-predicate dispatch the absent zephyr slot is contrasted with).
pub const CAP_LINUX_PCI_SURPRISE_1: CapabilityRow = CapabilityRow {
    id: "CAP-LINUX-PCI-SURPRISE-1",
    target_os: TargetOs::Linux,
    os_version_pin: "v6.6",
    bus_family: "pci",
    registration_form: RegistrationForm::PciDynamic,
    slot: LifecycleSlot::OnSurpriseRemove,
    disposition: Disposition::Delivered,
    citation: "present-predicate dispatch (pci_device_is_present) in `.remove`, \
               e1000-composed/golden.linux.c:370; v6.6 is the linux-backend docs' \
               verification pin (linux-open-questions.md, git.kernel.org ?h=v6.6 anchors)",
};

/// Every ratified row, in table order (C03).
pub const CAPABILITY_TABLE: &[&CapabilityRow] = &[
    &CAP_ZEPHYR_DT_SURPRISE_1,
    &CAP_ZEPHYR_DT_SHUTDOWN_1,
    &CAP_ZEPHYR_DT_DEINIT_1,
    &CAP_LINUX_PCI_SURPRISE_1,
];

/// Keyed lookup over the D02 key `(target_os, os_version_pin, bus_family,
/// registration_form, slot)`. A row whose `bus_family` is `"any"` matches
/// every queried bus.
pub fn lookup(
    target_os: TargetOs,
    os_version_pin: &str,
    bus_family: &str,
    registration_form: RegistrationForm,
    slot: LifecycleSlot,
) -> Option<&'static CapabilityRow> {
    CAPABILITY_TABLE.iter().copied().find(|row| {
        row.target_os == target_os
            && row.os_version_pin == os_version_pin
            && (row.bus_family == "any" || row.bus_family == bus_family)
            && row.registration_form == registration_form
            && row.slot == slot
    })
}

/// The per-target conformance report every OS-backend build emits
/// (amendment 6, C05). generic_c is exempt: it is the reference semantics
/// conformance is measured against; a self-report is vacuous (D09).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConformanceReport {
    pub target: &'static str, // "zephyr_c" | "linux_c"
    pub os_version_pin: &'static str,
    pub delivered_slots: Vec<SlotEntry>, // row: Some(id) iff a ratified row exists
    pub absent_dispatch_slots: Vec<SlotEntry>, // row: always Some; citation carried
    pub rejected_slots: Vec<SlotEntry>,  // structurally empty on successful emit
}

/// One slot row of the conformance report (C05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SlotEntry {
    pub slot: String,             // model slot name, e.g. "on_surprise_remove"
    pub row: Option<String>,      // CAP row ID
    pub citation: Option<String>, // present for absent_dispatch entries
}

impl ConformanceReport {
    /// The one ratified test-log line (guardrail 7 / R05; wording fixed at
    /// plan level): stable greppable prefix, target + pin bracketed, slots
    /// comma-separated in declaration order, row-bearing entries as
    /// `slot:ROW-ID`, empty lists rendered `[]`. The single source the JSON
    /// golden is also derived from (D08 guardrail 7).
    pub fn summary_line(&self) -> String {
        fn list(entries: &[SlotEntry]) -> String {
            let items: Vec<String> = entries
                .iter()
                .map(|e| match &e.row {
                    Some(row) => format!("{}:{row}", e.slot),
                    None => e.slot.clone(),
                })
                .collect();
            format!("[{}]", items.join(","))
        }
        format!(
            "RSEQ-CONFORMANCE [{} @ {}] delivered={} absent_dispatch={} rejected={}",
            self.target,
            self.os_version_pin,
            list(&self.delivered_slots),
            list(&self.absent_dispatch_slots),
            list(&self.rejected_slots)
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// C02 grammar + D02 key uniqueness, and the ID segments must agree
    /// with the row's own key fields (IDs are self-describing in
    /// diagnostics and C comments, D04).
    #[test]
    fn row_ids_follow_grammar_and_are_unique() {
        let mut seen_ids: Vec<&str> = Vec::new();
        let mut seen_keys: Vec<(TargetOs, &str, &str, RegistrationForm, LifecycleSlot)> =
            Vec::new();
        for row in CAPABILITY_TABLE {
            let parts: Vec<&str> = row.id.split('-').collect();
            assert_eq!(
                parts.len(),
                5,
                "row `{}`: the C02 grammar is CAP-<OS>-<FORM>-<SLOT>-<N>",
                row.id
            );
            assert_eq!(parts[0], "CAP", "row `{}`: prefix must be CAP", row.id);
            let os = match row.target_os {
                TargetOs::Linux => "LINUX",
                TargetOs::Zephyr => "ZEPHYR",
            };
            assert_eq!(parts[1], os, "row `{}`: <OS> must match target_os", row.id);
            let form = match row.registration_form {
                RegistrationForm::StaticDevicetree => "DT",
                RegistrationForm::PciDynamic => "PCI",
            };
            assert_eq!(
                parts[2], form,
                "row `{}`: <FORM> must match registration_form",
                row.id
            );
            let slot = match row.slot {
                LifecycleSlot::OnInit => "INIT",
                LifecycleSlot::OnInitFailure => "INITFAIL",
                LifecycleSlot::OnDeinit => "DEINIT",
                LifecycleSlot::OnSurpriseRemove => "SURPRISE",
                LifecycleSlot::OnSystemShutdown => "SHUTDOWN",
            };
            assert_eq!(parts[3], slot, "row `{}`: <SLOT> must match slot", row.id);
            let n: u32 = parts[4]
                .parse()
                .unwrap_or_else(|_| panic!("row `{}`: <N> must be a positive integer", row.id));
            assert!(n >= 1, "row `{}`: <N> must be positive", row.id);

            assert!(!seen_ids.contains(&row.id), "duplicate row id `{}`", row.id);
            seen_ids.push(row.id);
            let key = (
                row.target_os,
                row.os_version_pin,
                row.bus_family,
                row.registration_form,
                row.slot,
            );
            assert!(
                !seen_keys.contains(&key),
                "row `{}`: duplicate D02 key {key:?}",
                row.id
            );
            seen_keys.push(key);
        }
    }

    /// R09/I09: no table row may ever pair a NEVER_ABSENT slot with
    /// AbsentDispatch, on ANY OS.
    #[test]
    fn never_absent_slots_have_no_absent_rows() {
        for row in CAPABILITY_TABLE {
            if NEVER_ABSENT.contains(&row.slot) {
                assert_ne!(
                    row.disposition,
                    Disposition::AbsentDispatch,
                    "row `{}` pairs NEVER_ABSENT slot {:?} with AbsentDispatch",
                    row.id,
                    row.slot
                );
            }
        }
    }

    /// The mechanical half of the D05 citation bar (I04): non-empty and
    /// containing the row's own pin. The source-anchor half is human-
    /// verified at T02 and recorded in the citation text.
    #[test]
    fn absent_dispatch_citations_are_pinned() {
        for row in CAPABILITY_TABLE {
            if row.disposition == Disposition::AbsentDispatch {
                assert!(!row.citation.is_empty(), "row `{}`: empty citation", row.id);
                assert!(
                    row.citation.contains(row.os_version_pin),
                    "row `{}`: citation must contain its os_version_pin `{}`",
                    row.id,
                    row.os_version_pin
                );
            }
        }
    }

    /// The `"any"` bus wildcard over the D02 key: the static-DT surprise
    /// row must match every queried bus (incl. PCIe-over-DT — the zephyr
    /// E1000 case the key was designed for).
    #[test]
    fn lookup_bus_wildcard_matches_any_queried_bus() {
        for bus in ["any", "amba", "platform", "pci"] {
            let row = lookup(
                TargetOs::Zephyr,
                ZEPHYR_PIN,
                bus,
                RegistrationForm::StaticDevicetree,
                LifecycleSlot::OnSurpriseRemove,
            )
            .unwrap_or_else(|| panic!("wildcard row must match bus `{bus}`"));
            assert_eq!(row.id, CAP_ZEPHYR_DT_SURPRISE_1.id);
        }
        // A stale pin never matches (the evolution-audit seam, D12).
        assert!(lookup(
            TargetOs::Zephyr,
            "v9.9.9 (deadbeef)",
            "any",
            RegistrationForm::StaticDevicetree,
            LifecycleSlot::OnSurpriseRemove,
        )
        .is_none());
    }

    /// The R05 log line, wording fixed at plan level (Cross-cutting
    /// Workstreams): prefix, bracketed target@pin, `slot:ROW-ID` entries,
    /// `[]` for empty lists.
    #[test]
    fn summary_line_renders_the_ratified_wording() {
        let report = ConformanceReport {
            target: "zephyr_c",
            os_version_pin: ZEPHYR_PIN,
            delivered_slots: vec![SlotEntry {
                slot: "on_init".into(),
                row: None,
                citation: None,
            }],
            absent_dispatch_slots: vec![SlotEntry {
                slot: "on_surprise_remove".into(),
                row: Some(CAP_ZEPHYR_DT_SURPRISE_1.id.into()),
                citation: Some(CAP_ZEPHYR_DT_SURPRISE_1.citation.into()),
            }],
            rejected_slots: vec![],
        };
        assert_eq!(
            report.summary_line(),
            "RSEQ-CONFORMANCE [zephyr_c @ v4.4.0 (684c9e8f32e4)] delivered=[on_init] \
             absent_dispatch=[on_surprise_remove:CAP-ZEPHYR-DT-SURPRISE-1] rejected=[]"
        );
    }
}
