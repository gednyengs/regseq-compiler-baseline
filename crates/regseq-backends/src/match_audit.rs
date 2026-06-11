//! Mixed-match-set audit (spec-convergence-pl011 GRILL D01/D08, contract C02).
//!
//! The D03(a) rule (ratified backend-zephyr-c GRILL; operative form C06): a
//! backend must consume every NATIVE-transport match entry; may ignore
//! recognized FOREIGN-transport identity entries only when >=1 native entry
//! exists; must make the omission auditable; must reject
//! native-but-unsupported entries; must never synthesize transport-equivalent
//! entries. *Native* is defined per the backend's RATIFIED registration form
//! (D08) — linux_c+amba => `Amba`, linux_c+pci => `Pci`, zephyr_c (static-DT)
//! => `Devicetree` — NOT per OS (Linux-the-OS could carry an of_match_table;
//! the rule speaks of the registration form, so the audit wording must never
//! say "unsupported by this OS").
//!
//! One [`MatchAudit`] value renders into all three artifacts (D01, the
//! absent-slot precedent): the `ConformanceReport.ignored_match_entries`
//! section, the generated-C `RSEQ-IGNORED-MATCH` block ([`MatchAudit::c_block`],
//! C03), and the `RSEQ-CONFORMANCE` line's ` ignored=[...]` section —
//! cross-artifact consistency holds by construction.
//!
//! The license citation is this module's [`MATCH_AUDIT_LICENSE`] const — a
//! GRILL decision reference, deliberately NOT a capability-table row: the
//! capability table's domain stays lifecycle slots only
//! (os-capability-disposition boundary; plan decision 2 makes that boundary
//! structural by keeping this type out of the capability module).

use regseq_model::{MatchEntry, RegseqModel};

/// The fixed license citation every ignored-match audit carries (C02).
pub const MATCH_AUDIT_LICENSE: &str = "spec-convergence-pl011 GRILL D08 (D03(a) rule)";

/// One recognized foreign-transport identity entry a backend did not consume
/// (C02 as amended by A3: `native_on` carries the backend(s) whose ratified
/// registration form consumes the entry natively, so the C block's
/// "consumed natively by ..." clause renders from the same value as the
/// report row).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MatchAudit {
    /// Rendered entry, e.g. `devicetree compatible="regseq,pl011"` /
    /// `amba id=0x00041011 mask=0x000fffff`.
    pub entry: String,
    /// Always the fixed reason form ([`reason_for`]).
    pub reason: String,
    /// Always [`MATCH_AUDIT_LICENSE`].
    pub license: &'static str,
    /// Backend(s) whose ratified registration form consumes this entry
    /// natively (A3).
    pub native_on: &'static [&'static str],
}

impl MatchAudit {
    /// Build the audit value for one foreign entry under the given ratified
    /// registration form token (the backend's own form, e.g. `amba_driver`,
    /// `pci_driver`, `static-devicetree`).
    pub fn foreign(model: &RegseqModel, entry: &MatchEntry, ratified_form: &str) -> Self {
        MatchAudit {
            entry: render_entry(model, entry),
            reason: reason_for(ratified_form),
            license: MATCH_AUDIT_LICENSE,
            native_on: native_on(entry),
        }
    }

    /// The C03 `RSEQ-IGNORED-MATCH` C comment block, grammar verbatim —
    /// derived from the SAME struct fields as the report row (the reason's
    /// two clauses split at the fixed `"; "` of [`reason_for`]), so the
    /// same-struct guarantee stays literal.
    pub fn c_block(&self) -> String {
        let (head, tail) = self
            .reason
            .split_once("; ")
            .expect("fixed reason form (reason_for) always carries the two clauses");
        format!(
            "/* RSEQ-IGNORED-MATCH: {}\n * {}\n * ({}); consumed natively by {}.\n * [license: {}] */",
            self.entry,
            head,
            tail,
            self.native_on.join(", "),
            self.license
        )
    }
}

/// The fixed C02 reason form. Registration-form framing ONLY (D08): the
/// wording names the backend's ratified registration form, never the OS.
pub fn reason_for(ratified_form: &str) -> String {
    format!(
        "foreign transport: not consumed by the ratified {ratified_form} \
         registration form; >=1 native entry present"
    )
}

/// Which backend(s) consume this entry type natively, per the D08 nativeness
/// table: linux_c's ratified scaffolds consume `Amba`/`Pci`; zephyr_c's
/// static-DT form consumes `Devicetree`. Anything else (the deferred bus
/// families acpi/usb/i2c/spi) has NO ratified registration form anywhere —
/// it is NOT a licensed foreign entry and must keep the D.2 fail-loud
/// default (the bridge already rejects the attr sugar, CR-5; the backends
/// re-reject the model shape so synthetic models cannot slip through).
pub fn native_on(entry: &MatchEntry) -> &'static [&'static str] {
    match entry {
        MatchEntry::Amba { .. } | MatchEntry::Pci { .. } => &["linux_c"],
        MatchEntry::Devicetree { .. } => &["zephyr_c"],
        _ => &[],
    }
}

/// True iff the entry is in the D08 LICENSED foreign/native universe — i.e.
/// some backend's ratified registration form consumes it. The D03(a) rule
/// licenses ignoring only RECOGNIZED foreign-transport identity entries;
/// an entry with no ratified form on ANY backend is outside the rule and
/// keeps the fail-loud rejection (C06 final-review hardening).
pub fn has_ratified_form(entry: &MatchEntry) -> bool {
    !native_on(entry).is_empty()
}

/// Render a match entry in the fixed C02 spelling.
pub fn render_entry(model: &RegseqModel, entry: &MatchEntry) -> String {
    match entry {
        MatchEntry::Devicetree { compatible } => {
            format!(
                "devicetree compatible=\"{}\"",
                model.strings.resolve(*compatible)
            )
        }
        MatchEntry::Amba { id, mask } => format!("amba id={id:#010x} mask={mask:#010x}"),
        MatchEntry::Pci {
            vendor,
            device,
            subsystem,
            class,
        } => {
            let mut out = format!("pci vendor={vendor:#06x} device={device:#06x}");
            if let Some(s) = subsystem {
                out.push_str(&format!(" subsystem={s:#010x}"));
            }
            if let Some(c) = class {
                out.push_str(&format!(" class={c:#08x}"));
            }
            out
        }
        other => format!("{other:?}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_model::StableCrateId;

    fn dt_audit() -> (RegseqModel, MatchAudit) {
        let mut model = RegseqModel::with_primary("match_audit_t", StableCrateId(1));
        let compatible = model.strings.intern("regseq,pl011");
        let entry = MatchEntry::Devicetree { compatible };
        let audit = MatchAudit::foreign(&model, &entry, "amba_driver");
        (model, audit)
    }

    #[test]
    fn foreign_dt_entry_renders_the_fixed_forms() {
        let (_, audit) = dt_audit();
        assert_eq!(audit.entry, "devicetree compatible=\"regseq,pl011\"");
        assert_eq!(
            audit.reason,
            "foreign transport: not consumed by the ratified amba_driver \
             registration form; >=1 native entry present"
        );
        assert_eq!(audit.license, MATCH_AUDIT_LICENSE);
        assert_eq!(audit.native_on, &["zephyr_c"]);
    }

    #[test]
    fn c_block_grammar_is_verbatim_from_the_same_struct() {
        let (_, audit) = dt_audit();
        assert_eq!(
            audit.c_block(),
            "/* RSEQ-IGNORED-MATCH: devicetree compatible=\"regseq,pl011\"\n \
             * foreign transport: not consumed by the ratified amba_driver registration form\n \
             * (>=1 native entry present); consumed natively by zephyr_c.\n \
             * [license: spec-convergence-pl011 GRILL D08 (D03(a) rule)] */"
        );
    }

    #[test]
    fn amba_and_pci_entries_render_native_on_linux() {
        let model = RegseqModel::with_primary("match_audit_t", StableCrateId(1));
        let amba = MatchEntry::Amba {
            id: 0x0004_1011,
            mask: 0x000f_ffff,
        };
        let audit = MatchAudit::foreign(&model, &amba, "static-devicetree");
        assert_eq!(audit.entry, "amba id=0x00041011 mask=0x000fffff");
        assert_eq!(audit.native_on, &["linux_c"]);

        let pci = MatchEntry::Pci {
            vendor: 0x8086,
            device: 0x100e,
            subsystem: Some(0x1234_5678),
            class: None,
        };
        assert_eq!(
            render_entry(&model, &pci),
            "pci vendor=0x8086 device=0x100e subsystem=0x12345678"
        );
        assert_eq!(native_on(&pci), &["linux_c"]);
    }
}
