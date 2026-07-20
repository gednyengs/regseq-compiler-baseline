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

/// The closed disposition enum (amendment 1 as amended, D01; appended by the
/// device-interrupt unit per its GRILL D03/C02 — O5 minted here).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Disposition {
    Delivered,
    AbsentDispatch,    // ONLY this variant may non-emit an authored callback
    DesignedScheduled, // slot witnessed/designed; emission scheduled behind a
    // named pin or successor; reject-at-emit with the
    // scheduling diagnostic (never the word "unsupported")
    NeedsNewSurface,     // model/fixture gap; escalate, blocks
    RejectedUnsupported, // fail-loud D.2 default
    /// A compiler-managed OPERATION (not an authored callback) that is
    /// structurally absent on this registration form (device-interrupt GRILL
    /// D03: zephyr's static `IRQ_CONNECT` has no runtime disconnect — the
    /// deregister slot cannot be emitted; `irq_disable` still is, leaving the
    /// line connected-but-masked). Distinct from [`Disposition::AbsentDispatch`]
    /// by domain: that licenses non-emission of an authored LIFECYCLE callback
    /// whose dispatch cannot occur; this licenses non-emission of a managed
    /// operation the form cannot express. Renders its own audit-block kind
    /// (`RSEQ-ABSENT-OP`), never the `RSEQ-ABSENT-SLOT` spelling.
    StructurallyAbsent,
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

/// The widened slot domain (device-interrupt unit, C02/R07): a row's subject
/// is either an authored lifecycle callback slot or a compiler-managed
/// OPERATION slot in dotted-path spelling. Existing lifecycle rows keep their
/// rendered forms byte-identical (the `Lifecycle` arm delegates to
/// [`LifecycleSlot::model_name`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CapabilitySlot {
    /// An authored lifecycle callback slot.
    Lifecycle(LifecycleSlot),
    /// A compiler-managed operation slot, dotted-path spelling. First
    /// occupant: `interrupt.register_handler.deregister` (construct #6).
    Operation(&'static str),
}

impl CapabilitySlot {
    /// The rendered slot name — the spelling every artifact carries.
    pub fn model_name(self) -> &'static str {
        match self {
            CapabilitySlot::Lifecycle(slot) => slot.model_name(),
            CapabilitySlot::Operation(path) => path,
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
    pub slot: CapabilitySlot,
    pub disposition: Disposition,
    /// Non-empty; must contain the pin + >=1 source anchor in the pinned
    /// local checkout or vendored reference (D05; L3 empty-ratified rule).
    pub citation: &'static str,
}

impl CapabilityRow {
    /// The C04 audit block for this row, grammar verbatim — generated from
    /// the same const as the report row, so cross-artifact row-ID consistency
    /// holds by construction (D10). Each absent-family disposition renders
    /// its OWN kind spelling (device-interrupt C02: the generalization that
    /// retired the hard-coded `AbsentDispatch` text): `AbsentDispatch` keeps
    /// its historical `RSEQ-ABSENT-SLOT` block byte-identical;
    /// `StructurallyAbsent` renders `RSEQ-ABSENT-OP` with the
    /// connected-but-masked operation tail.
    pub fn audit_block(&self) -> String {
        match self.disposition {
            Disposition::AbsentDispatch => format!(
                "/* RSEQ-ABSENT-SLOT: {}\n * disposition: AbsentDispatch row: {}\n * [ratified: {}]\n * body checked, not delivered: dispatch cannot occur on this target. */\n",
                self.slot.model_name(),
                self.id,
                self.citation
            ),
            Disposition::StructurallyAbsent => format!(
                "/* RSEQ-ABSENT-OP: {}\n * disposition: StructurallyAbsent row: {}\n * [ratified: {}]\n * operation not emitted: structurally absent on this registration form. */\n",
                self.slot.model_name(),
                self.id,
                self.citation
            ),
            other => unreachable!(
                "audit_block is rendered only for the absent-family dispositions; \
                 row `{}` carries {other:?}",
                self.id
            ),
        }
    }
}

/// Single authoritative current pin (D12) — the one place a re-pin edits.
pub const ZEPHYR_PIN: &str = "v4.4.0 (684c9e8f32e4)";

/// The Linux verification pin the linux-backend docs were verified against
/// (linux-open-questions.md, git.kernel.org `?h=v6.6` anchors). Kept here
/// for the same one-place-edit reason as [`ZEPHYR_PIN`]; the amendment-9
/// staleness mechanics are ratified for Zephyr rows only (D12).
pub const LINUX_PIN: &str = "v6.6";

/// Slots that may NEVER carry Disposition::AbsentDispatch, on any OS (D14:
/// "probe cannot fail" is almost certainly false once allocation/mapping/
/// registration are included).
pub const NEVER_ABSENT: &[LifecycleSlot] = &[LifecycleSlot::OnInit, LifecycleSlot::OnInitFailure];

/// Zephyr static-DT `on_surprise_remove`: the one ratified absent slot
/// (C03 row 1; resolves B-7 — answer: not representable, ratified).
pub const CAP_ZEPHYR_DT_SURPRISE_1: CapabilityRow = CapabilityRow {
    id: "CAP-ZEPHYR-DT-SURPRISE-1",
    target_os: TargetOs::Zephyr,
    // Deliberate LITERAL, never `ZEPHYR_PIN`: a row records the pin its
    // claim was RATIFIED against. Referencing the current-pin const would
    // make every row track a re-pin automatically and the amendment-9
    // staleness audit could never fire (Codex M2 review finding 1).
    os_version_pin: "v4.4.0 (684c9e8f32e4)",
    bus_family: "any", // incl. PCIe-over-DT (D02: the FORM carries the claim)
    registration_form: RegistrationForm::StaticDevicetree,
    slot: CapabilitySlot::Lifecycle(LifecycleSlot::OnSurpriseRemove),
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
    os_version_pin: "v4.4.0 (684c9e8f32e4)", // ratified literal (see SURPRISE-1)
    bus_family: "any",
    registration_form: RegistrationForm::StaticDevicetree,
    slot: CapabilitySlot::Lifecycle(LifecycleSlot::OnSystemShutdown),
    disposition: Disposition::DesignedScheduled,
    citation: "PM-device slot witnessed, wrapper designed (scaffold §4); trigger contract \
               pending C-4 pin, per A-18",
};

/// Zephyr `on_deinit`: DELIVERED by spec-convergence-pl011 (C03 row 3,
/// flipped IN PLACE per its GRILL D05 — no N+1: this unit delivers exactly
/// what the -1 row scheduled, as pre-registered).
pub const CAP_ZEPHYR_DT_DEINIT_1: CapabilityRow = CapabilityRow {
    id: "CAP-ZEPHYR-DT-DEINIT-1",
    target_os: TargetOs::Zephyr,
    os_version_pin: "v4.4.0 (684c9e8f32e4)", // ratified literal (see SURPRISE-1)
    bus_family: "any",
    registration_form: RegistrationForm::StaticDevicetree,
    slot: CapabilitySlot::Lifecycle(LifecycleSlot::OnDeinit),
    disposition: Disposition::Delivered,
    citation: "`device_deinit()` device.h:928; `DEVICE_DT_DEINIT_DEFINE` device.h:243; \
               rc returned verbatim kernel/device.c:199; `CONFIG_DEVICE_DEINIT_SUPPORT` \
               kernel/Kconfig.device:36; delivered by spec-convergence-pl011 GRILL D03",
};

/// Linux PCI `on_surprise_remove`: delivered (C03 row 4 — the
/// present-predicate dispatch the absent zephyr slot is contrasted with).
pub const CAP_LINUX_PCI_SURPRISE_1: CapabilityRow = CapabilityRow {
    id: "CAP-LINUX-PCI-SURPRISE-1",
    target_os: TargetOs::Linux,
    os_version_pin: "v6.6", // ratified literal (see SURPRISE-1)
    bus_family: "pci",
    registration_form: RegistrationForm::PciDynamic,
    slot: CapabilitySlot::Lifecycle(LifecycleSlot::OnSurpriseRemove),
    disposition: Disposition::Delivered,
    citation: "present-predicate dispatch (pci_device_is_present) in `.remove`, \
               e1000-composed/golden.linux.c:370; v6.6 is the linux-backend docs' \
               verification pin (linux-open-questions.md, git.kernel.org ?h=v6.6 anchors)",
};

/// Zephyr static-`IRQ_CONNECT` interrupt deregister: the table's FIRST
/// non-lifecycle row (device-interrupt construct #6, GRILL D03/C02). The
/// operation — not an authored callback — is structurally absent on the
/// static-connect registration form; `irq_disable` IS emitted and the line is
/// left connected-but-masked after deinit.
pub const CAP_ZEPHYR_STATICIRQ_DEREGISTER_1: CapabilityRow = CapabilityRow {
    id: "CAP-ZEPHYR-STATICIRQ-DEREGISTER-1",
    target_os: TargetOs::Zephyr,
    os_version_pin: "v4.4.0 (684c9e8f32e4)", // ratified literal (see SURPRISE-1)
    bus_family: "any",
    registration_form: RegistrationForm::StaticDevicetree,
    slot: CapabilitySlot::Operation("interrupt.register_handler.deregister"),
    disposition: Disposition::StructurallyAbsent,
    citation: "Zephyr v4.4.0 (684c9e8f32e4) static IRQ_CONNECT, re-verified live in the \
               eval-workspace checkout 2026-06-11 (device-interrupt T10): IRQ_CONNECT \
               (include/zephyr/irq.h:48) expands to ARCH_IRQ_CONNECT, a BUILD-TIME \
               sw_isr_table install; the only disconnect API is irq_disconnect_dynamic \
               (irq.h:90), documented for irq_connect_dynamic clients (z_isr_uninstall \
               sw_isr_table.h:302 is its dynamic-table internal) — no runtime disconnect \
               exists for a static connect. irq_disable IS emitted at deinit; the line \
               stays connected-but-masked (GRILL device-interrupt D03/C02)",
};

/// Every ratified row, in table order (C03).
pub const CAPABILITY_TABLE: &[&CapabilityRow] = &[
    &CAP_ZEPHYR_DT_SURPRISE_1,
    &CAP_ZEPHYR_DT_SHUTDOWN_1,
    &CAP_ZEPHYR_DT_DEINIT_1,
    &CAP_LINUX_PCI_SURPRISE_1,
    &CAP_ZEPHYR_STATICIRQ_DEREGISTER_1,
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
            && row.slot == CapabilitySlot::Lifecycle(slot)
    })
}

/// The fixed license citation the deferred-class disposition carries
/// (backend-zephyr-c-e1000 D14/C06) — a GRILL decision reference,
/// deliberately NOT a capability-table row (the table's domain stays
/// lifecycle slots only). The TU audit comment and the conformance note
/// render from this same const, so cross-artifact consistency holds by
/// construction.
pub const DEFERRED_CLASS_LICENSE: &str = "backend-zephyr-c-e1000 GRILL D14";

/// The OPTIONAL deferred-class note (backend-zephyr-c-e1000 C06 item 2):
/// carried when the spec declares a non-Generic device class that registers
/// class-neutral under the D14 audited disposition. Skip-serialized when
/// absent, so every conformance artifact emitted before this disposition
/// existed stays byte-identical.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeferredClassNote {
    /// The declared class name, e.g. "network".
    pub class: String,
    /// Always [`DEFERRED_CLASS_LICENSE`].
    pub license: &'static str,
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
    /// Compiler-managed operations structurally absent on the registration
    /// form (device-interrupt C02): row always Some, citation carried — the
    /// connected-but-masked statement lives in the row's citation.
    pub structurally_absent_slots: Vec<SlotEntry>,
    pub rejected_slots: Vec<SlotEntry>, // structurally empty on successful emit
    /// Recognized foreign-transport identity entries the backend did not
    /// consume, per the D03(a) rule (GRILL spec-convergence-pl011 D08, C02).
    /// Empty when the match set is all-native. NOT a slot section: the
    /// license cites the GRILL decision const, never a capability row (the
    /// table's domain stays lifecycle slots only).
    pub ignored_match_entries: Vec<crate::match_audit::MatchAudit>,
    /// The C06 deferred-class note — `Some` only when a non-Generic class
    /// registered class-neutral (backend-zephyr-c-e1000 D14).
    pub deferred_class: Option<DeferredClassNote>,
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
        // The ` ignored=[...]` section (spec-convergence-pl011 C02): rendered
        // entries `;`-separated, `ignored=[]` when empty; the existing
        // prefix/format stays byte-stable.
        let ignored: Vec<&str> = self
            .ignored_match_entries
            .iter()
            .map(|a| a.entry.as_str())
            .collect();
        // The deferred-class section appears ONLY when the note is present
        // (C06 item 2): the existing line shape stays byte-stable.
        let deferred = self
            .deferred_class
            .as_ref()
            .map(|n| format!(" deferred_class=[{}]", n.class))
            .unwrap_or_default();
        format!(
            "RSEQ-CONFORMANCE [{} @ {}] delivered={} absent_dispatch={} structurally_absent={} rejected={} ignored=[{}]{deferred}",
            self.target,
            self.os_version_pin,
            list(&self.delivered_slots),
            list(&self.absent_dispatch_slots),
            list(&self.structurally_absent_slots),
            list(&self.rejected_slots),
            ignored.join(";")
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // -- the C08 audit suite: doc-mirror sync (D03) ---------------------------

    /// The doc-mirror spelling of a registration form.
    fn doc_token_registration(form: RegistrationForm) -> &'static str {
        match form {
            RegistrationForm::StaticDevicetree => "static-devicetree",
            RegistrationForm::PciDynamic => "pci-dynamic",
        }
    }

    /// The doc-mirror spelling of a disposition.
    fn doc_token_disposition(d: Disposition) -> &'static str {
        match d {
            Disposition::Delivered => "Delivered",
            Disposition::AbsentDispatch => "AbsentDispatch",
            Disposition::DesignedScheduled => "DesignedScheduled",
            Disposition::NeedsNewSurface => "NeedsNewSurface",
            Disposition::RejectedUnsupported => "RejectedUnsupported",
            Disposition::StructurallyAbsent => "StructurallyAbsent",
        }
    }

    /// The canonical mirror line for a row (one row per line, fixed
    /// columns — kept machine-trivial like the Doc-1 matrix).
    fn doc_line(row: &CapabilityRow) -> String {
        format!(
            "| {} | {} | {} | {} | {} | {} | {} | {} |",
            row.id,
            row.target_os.token(),
            row.os_version_pin,
            row.bus_family,
            doc_token_registration(row.registration_form),
            row.slot.model_name(),
            doc_token_disposition(row.disposition),
            row.citation
        )
    }

    /// The canonical NEVER_ABSENT mirror line.
    fn never_absent_line() -> String {
        format!(
            "NEVER_ABSENT: {}",
            NEVER_ABSENT
                .iter()
                .map(|s| s.model_name())
                .collect::<Vec<_>>()
                .join(", ")
        )
    }

    /// Code↔doc mirror sync (D03): every code row's (ID, key fields,
    /// disposition, pin, citation) verbatim in the doc; no doc row without
    /// a code row; NEVER_ABSENT mirrored. `Err` names the FIRST diverging
    /// field (the identity.rs `first_divergence` discipline).
    fn sync_check(doc: &str) -> Result<(), String> {
        let mut doc_rows: Vec<Vec<String>> = Vec::new();
        for line in doc.lines() {
            let line = line.trim();
            if !line.starts_with('|') {
                continue;
            }
            let cells: Vec<String> = line
                .trim_matches('|')
                .split('|')
                .map(|c| c.trim().to_string())
                .collect();
            if cells.first().is_some_and(|c| c.starts_with("CAP-")) {
                doc_rows.push(cells);
            }
        }
        for row in CAPABILITY_TABLE {
            let doc_row = doc_rows
                .iter()
                .find(|c| c[0] == row.id)
                .ok_or_else(|| format!("row `{}`: missing from the doc mirror", row.id))?;
            if doc_row.len() != 8 {
                return Err(format!(
                    "row `{}`: doc row has {} cells, expected 8 (id, target os, pin, bus, \
                     registration, slot, disposition, citation)",
                    row.id,
                    doc_row.len()
                ));
            }
            let fields: [(&str, &str, &str); 7] = [
                ("target_os", row.target_os.token(), &doc_row[1]),
                ("os_version_pin", row.os_version_pin, &doc_row[2]),
                ("bus_family", row.bus_family, &doc_row[3]),
                (
                    "registration_form",
                    doc_token_registration(row.registration_form),
                    &doc_row[4],
                ),
                ("slot", row.slot.model_name(), &doc_row[5]),
                (
                    "disposition",
                    doc_token_disposition(row.disposition),
                    &doc_row[6],
                ),
                ("citation", row.citation, &doc_row[7]),
            ];
            for (name, code, doc_v) in fields {
                if code != doc_v {
                    return Err(format!(
                        "row `{}`: field `{name}` diverges — code: `{code}` / doc: `{doc_v}`",
                        row.id
                    ));
                }
            }
        }
        for cells in &doc_rows {
            if !CAPABILITY_TABLE.iter().any(|r| r.id == cells[0]) {
                return Err(format!(
                    "doc row `{}` has no code row (the table is append-only through \
                     ratification, never doc-first)",
                    cells[0]
                ));
            }
        }
        let na = never_absent_line();
        if !doc.contains(&na) {
            return Err(format!("doc mirror missing the `{na}` line"));
        }
        Ok(())
    }

    /// Every Zephyr AbsentDispatch/DesignedScheduled/StructurallyAbsent row
    /// whose pin is not `pin` — the amendment-9 staleness detector, naming
    /// each stale row. StructurallyAbsent joined the family with the
    /// device-interrupt unit (Codex M4 review): its impossibility claim is
    /// just as version-bound as AbsentDispatch's, so a re-pin must re-verify
    /// it too.
    fn stale_zephyr_rows<'a>(
        table: impl Iterator<Item = &'a CapabilityRow>,
        pin: &str,
    ) -> Vec<&'a str> {
        table
            .filter(|r| {
                r.target_os == TargetOs::Zephyr
                    && matches!(
                        r.disposition,
                        Disposition::AbsentDispatch
                            | Disposition::DesignedScheduled
                            | Disposition::StructurallyAbsent
                    )
                    && r.os_version_pin != pin
            })
            .map(|r| r.id)
            .collect()
    }

    #[test]
    fn capability_table_matches_ratified_doc() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/discussions/os-capability-disposition/capability-table.md");
        let doc = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("cannot read the ratified mirror {}: {e}", path.display()));
        if let Err(msg) = sync_check(&doc) {
            panic!("capability table / doc mirror out of sync — {msg}");
        }
    }

    #[test]
    fn capability_doc_sync_names_the_first_diverging_field() {
        // A doc whose surprise row carries a stale pin: the failure must
        // name `os_version_pin` (the first diverging field), not a generic
        // mismatch.
        let tampered = CapabilityRow {
            os_version_pin: "v4.3.0 (0000000000ab)",
            ..clone_row(&CAP_ZEPHYR_DT_SURPRISE_1)
        };
        let mut doc = String::new();
        doc.push_str(&doc_line(&tampered));
        doc.push('\n');
        for row in &CAPABILITY_TABLE[1..] {
            doc.push_str(&doc_line(row));
            doc.push('\n');
        }
        doc.push_str(&never_absent_line());
        doc.push('\n');
        let err = sync_check(&doc).unwrap_err();
        assert!(err.contains("field `os_version_pin`"), "{err}");
        assert!(err.contains("CAP-ZEPHYR-DT-SURPRISE-1"), "{err}");

        // A doc row with no code row fails naming the orphan.
        let mut doc = full_doc();
        doc.push_str("| CAP-LINUX-PCI-INIT-9 | linux | v6.6 | pci | pci-dynamic | on_init | Delivered | bogus |\n");
        let err = sync_check(&doc).unwrap_err();
        assert!(err.contains("CAP-LINUX-PCI-INIT-9"), "{err}");

        // A missing code row fails naming it.
        let doc = full_doc().replace(&doc_line(&CAP_ZEPHYR_DT_DEINIT_1), "");
        let err = sync_check(&doc).unwrap_err();
        assert!(
            err.contains("CAP-ZEPHYR-DT-DEINIT-1") && err.contains("missing"),
            "{err}"
        );
    }

    /// A field-for-field copy (CapabilityRow is deliberately not Clone:
    /// rows exist as ratified consts, never constructed downstream).
    fn clone_row(r: &CapabilityRow) -> CapabilityRow {
        CapabilityRow {
            id: r.id,
            target_os: r.target_os,
            os_version_pin: r.os_version_pin,
            bus_family: r.bus_family,
            registration_form: r.registration_form,
            slot: r.slot,
            disposition: r.disposition,
            citation: r.citation,
        }
    }

    /// The fully-canonical in-memory mirror (rows + NEVER_ABSENT).
    fn full_doc() -> String {
        let mut doc = String::new();
        for row in CAPABILITY_TABLE {
            doc.push_str(&doc_line(row));
            doc.push('\n');
        }
        doc.push_str(&never_absent_line());
        doc.push('\n');
        doc
    }

    #[test]
    fn absent_dispatch_rows_match_current_os_pin() {
        // R08 / amendment 9: a re-pin (ZEPHYR_PIN edit) without per-row
        // re-review fails NAMING each stale row.
        let stale = stale_zephyr_rows(CAPABILITY_TABLE.iter().copied(), ZEPHYR_PIN);
        assert!(
            stale.is_empty(),
            "zephyr rows pinned to a superseded version — re-verify each on the current \
             ZEPHYR_PIN and update its pin+citation (amendment 9): {stale:?}"
        );
        // The detector names every stale row (witnessed on a synthetic
        // stale table).
        let stale_row = CapabilityRow {
            os_version_pin: "v4.3.0 (0000000000ab)",
            ..clone_row(&CAP_ZEPHYR_DT_SURPRISE_1)
        };
        let table = [&stale_row, &CAP_ZEPHYR_DT_SHUTDOWN_1];
        let stale = stale_zephyr_rows(table.into_iter(), ZEPHYR_PIN);
        assert_eq!(stale, vec!["CAP-ZEPHYR-DT-SURPRISE-1"]);
    }

    /// C02 grammar + D02 key uniqueness, and the ID segments must agree
    /// with the row's own key fields (IDs are self-describing in
    /// diagnostics and C comments, D04).
    #[test]
    fn row_ids_follow_grammar_and_are_unique() {
        let mut seen_ids: Vec<&str> = Vec::new();
        let mut seen_keys: Vec<(TargetOs, &str, &str, RegistrationForm, CapabilitySlot)> =
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
            // The <FORM>/<SLOT> segment pair. Lifecycle rows derive both from
            // the enum fields; an Operation row's pair is RATIFIED with the
            // row (the device-interrupt GRILL D03 names
            // CAP-ZEPHYR-STATICIRQ-DEREGISTER-1: <FORM> = the static
            // IRQ_CONNECT registration spelling, expressed by the SAME
            // RegistrationForm::StaticDevicetree — no duplicate enum form is
            // minted; the segment carries the operation-family precision).
            let (form, slot) = match row.slot {
                CapabilitySlot::Lifecycle(lifecycle) => {
                    let form = match row.registration_form {
                        RegistrationForm::StaticDevicetree => "DT",
                        RegistrationForm::PciDynamic => "PCI",
                    };
                    let slot = match lifecycle {
                        LifecycleSlot::OnInit => "INIT",
                        LifecycleSlot::OnInitFailure => "INITFAIL",
                        LifecycleSlot::OnDeinit => "DEINIT",
                        LifecycleSlot::OnSurpriseRemove => "SURPRISE",
                        LifecycleSlot::OnSystemShutdown => "SHUTDOWN",
                    };
                    (form, slot)
                }
                CapabilitySlot::Operation("interrupt.register_handler.deregister") => {
                    ("STATICIRQ", "DEREGISTER")
                }
                CapabilitySlot::Operation(other) => panic!(
                    "row `{}`: operation slot `{other}` has no ratified ID segment pair",
                    row.id
                ),
            };
            assert_eq!(
                parts[2], form,
                "row `{}`: <FORM> must match the ratified form segment",
                row.id
            );
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
            if let CapabilitySlot::Lifecycle(slot) = row.slot {
                if NEVER_ABSENT.contains(&slot) {
                    assert_ne!(
                        row.disposition,
                        Disposition::AbsentDispatch,
                        "row `{}` pairs NEVER_ABSENT slot {:?} with AbsentDispatch",
                        row.id,
                        slot
                    );
                }
            }
        }
    }

    /// The mechanical half of the D05 citation bar (I04): non-empty and
    /// containing the row's own pin. The source-anchor half is human-
    /// verified at T02 and recorded in the citation text.
    #[test]
    fn absent_dispatch_citations_are_pinned() {
        for row in CAPABILITY_TABLE {
            if matches!(
                row.disposition,
                Disposition::AbsentDispatch | Disposition::StructurallyAbsent
            ) {
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
            structurally_absent_slots: vec![SlotEntry {
                slot: CAP_ZEPHYR_STATICIRQ_DEREGISTER_1.slot.model_name().into(),
                row: Some(CAP_ZEPHYR_STATICIRQ_DEREGISTER_1.id.into()),
                citation: Some(CAP_ZEPHYR_STATICIRQ_DEREGISTER_1.citation.into()),
            }],
            rejected_slots: vec![],
            ignored_match_entries: vec![],
            deferred_class: None,
        };
        assert_eq!(
            report.summary_line(),
            "RSEQ-CONFORMANCE [zephyr_c @ v4.4.0 (684c9e8f32e4)] delivered=[on_init] \
             absent_dispatch=[on_surprise_remove:CAP-ZEPHYR-DT-SURPRISE-1] \
             structurally_absent=[interrupt.register_handler.deregister:CAP-ZEPHYR-STATICIRQ-DEREGISTER-1] \
             rejected=[] ignored=[]"
        );
    }

    /// C02 (spec-convergence-pl011): a carried ignored entry renders into the
    /// line's trailing section, `;`-separated, from the same struct.
    #[test]
    fn summary_line_renders_ignored_match_entries() {
        let report = ConformanceReport {
            target: "zephyr_c",
            os_version_pin: ZEPHYR_PIN,
            delivered_slots: vec![],
            absent_dispatch_slots: vec![],
            structurally_absent_slots: vec![],
            rejected_slots: vec![],
            ignored_match_entries: vec![
                crate::match_audit::MatchAudit {
                    entry: "amba id=0x00041011 mask=0x000fffff".into(),
                    reason: crate::match_audit::reason_for("static-devicetree"),
                    license: crate::match_audit::MATCH_AUDIT_LICENSE,
                    native_on: &["linux_c"],
                },
                crate::match_audit::MatchAudit {
                    entry: "amba id=0x00abc123 mask=0x00ffffff".into(),
                    reason: crate::match_audit::reason_for("static-devicetree"),
                    license: crate::match_audit::MATCH_AUDIT_LICENSE,
                    native_on: &["linux_c"],
                },
            ],
            deferred_class: None,
        };
        assert_eq!(
            report.summary_line(),
            "RSEQ-CONFORMANCE [zephyr_c @ v4.4.0 (684c9e8f32e4)] delivered=[] \
             absent_dispatch=[] structurally_absent=[] rejected=[] \
             ignored=[amba id=0x00041011 mask=0x000fffff;amba id=0x00abc123 mask=0x00ffffff]"
        );
    }

    /// C06 item 2 (backend-zephyr-c-e1000): the deferred-class section
    /// appears in the summary line ONLY when the note is present; the line
    /// without it is byte-identical to today's shape (the two tests above).
    #[test]
    fn summary_line_gains_a_deferred_class_section_only_when_present() {
        let report = ConformanceReport {
            target: "zephyr_c",
            os_version_pin: ZEPHYR_PIN,
            delivered_slots: vec![],
            absent_dispatch_slots: vec![],
            structurally_absent_slots: vec![],
            rejected_slots: vec![],
            ignored_match_entries: vec![],
            deferred_class: Some(DeferredClassNote {
                class: "network".into(),
                license: DEFERRED_CLASS_LICENSE,
            }),
        };
        assert_eq!(
            report.summary_line(),
            "RSEQ-CONFORMANCE [zephyr_c @ v4.4.0 (684c9e8f32e4)] delivered=[] \
             absent_dispatch=[] structurally_absent=[] rejected=[] ignored=[] \
             deferred_class=[network]"
        );
    }
}
