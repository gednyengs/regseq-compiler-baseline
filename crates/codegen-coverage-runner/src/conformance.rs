//! Deterministic JSON rendering of the per-target [`ConformanceReport`]
//! (os-capability-disposition amendment 6, C05).
//!
//! The backends stay serde-free (the report is a plain struct on
//! `ZephyrEmit`/`LinuxEmit`); the runner owns the serialized shape. The
//! rendering is byte-deterministic by construction: fixed field order
//! (target, os_version_pin, delivered_slots, absent_dispatch_slots,
//! structurally_absent_slots, rejected_slots, ignored_match_entries), slots
//! in the struct's declaration
//! order, no wall-clock. Empty lists render `[]`; a `citation` key appears
//! only on entries that carry one (the C05 example shape); the
//! `ignored_match_entries` key is serialized UNCONDITIONALLY
//! (spec-convergence-pl011 C02). The committed `conformance.<os>.json`
//! golden beside each OS-target fixture is this output verbatim.

use regseq_backends::capability::{ConformanceReport, SlotEntry};
use regseq_backends::MatchAudit;

/// Render the C05 JSON, byte-stable across runs.
pub fn render_json(report: &ConformanceReport) -> String {
    format!(
        "{{\n  \"target\": {},\n  \"os_version_pin\": {},\n{},\n{},\n{},\n{},\n{}\n}}\n",
        json_str(report.target),
        json_str(report.os_version_pin),
        render_list("delivered_slots", &report.delivered_slots),
        render_list("absent_dispatch_slots", &report.absent_dispatch_slots),
        render_list("structurally_absent_slots", &report.structurally_absent_slots),
        render_list("rejected_slots", &report.rejected_slots),
        render_audit_list(&report.ignored_match_entries),
    )
}

/// The `ignored_match_entries` section (spec-convergence-pl011 C02):
/// serialized unconditionally, every field of each [`MatchAudit`] rendered.
fn render_audit_list(entries: &[MatchAudit]) -> String {
    if entries.is_empty() {
        return "  \"ignored_match_entries\": []".to_string();
    }
    let items: Vec<String> = entries
        .iter()
        .map(|a| {
            let native_on: Vec<String> = a.native_on.iter().map(|b| json_str(b)).collect();
            format!(
                "    {{\"entry\": {}, \"reason\": {}, \"license\": {}, \"native_on\": [{}]}}",
                json_str(&a.entry),
                json_str(&a.reason),
                json_str(a.license),
                native_on.join(", ")
            )
        })
        .collect();
    format!(
        "  \"ignored_match_entries\": [\n{}\n  ]",
        items.join(",\n")
    )
}

fn render_list(key: &str, entries: &[SlotEntry]) -> String {
    if entries.is_empty() {
        return format!("  \"{key}\": []");
    }
    let items: Vec<String> = entries
        .iter()
        .map(|e| format!("    {}", render_entry(e)))
        .collect();
    format!("  \"{key}\": [\n{}\n  ]", items.join(",\n"))
}

fn render_entry(e: &SlotEntry) -> String {
    let row = match &e.row {
        Some(r) => json_str(r),
        None => "null".to_string(),
    };
    let mut out = format!("{{\"slot\": {}, \"row\": {row}", json_str(&e.slot));
    if let Some(c) = &e.citation {
        out.push_str(&format!(", \"citation\": {}", json_str(c)));
    }
    out.push('}');
    out
}

/// Minimal JSON string escaping (RFC 8259): quote, backslash, control
/// characters. Slot names, row IDs, pins and citations are plain prose,
/// but the renderer never trusts that.
fn json_str(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use regseq_backends::capability;

    fn sample() -> ConformanceReport {
        ConformanceReport {
            target: "zephyr_c",
            os_version_pin: capability::ZEPHYR_PIN,
            delivered_slots: vec![SlotEntry {
                slot: "on_init".into(),
                row: None,
                citation: None,
            }],
            absent_dispatch_slots: vec![SlotEntry {
                slot: "on_surprise_remove".into(),
                row: Some(capability::CAP_ZEPHYR_DT_SURPRISE_1.id.into()),
                citation: Some("ratified citation \"quoted\"".into()),
            }],
            structurally_absent_slots: vec![],
            rejected_slots: vec![],
            ignored_match_entries: vec![],
        }
    }

    /// I05: the renderer is byte-deterministic (render twice, compare) and
    /// the shape matches the C05 example: fixed field order, null rows,
    /// citation key only where carried, `[]` for empty lists (including the
    /// unconditional `ignored_match_entries` key, spec-convergence-pl011 C02).
    #[test]
    fn render_is_deterministic_and_matches_the_c05_shape() {
        let report = sample();
        let a = render_json(&report);
        let b = render_json(&report);
        assert_eq!(a, b, "byte-stable across runs");
        assert_eq!(
            a,
            "{\n  \"target\": \"zephyr_c\",\n  \"os_version_pin\": \"v4.4.0 (684c9e8f32e4)\",\n  \"delivered_slots\": [\n    {\"slot\": \"on_init\", \"row\": null}\n  ],\n  \"absent_dispatch_slots\": [\n    {\"slot\": \"on_surprise_remove\", \"row\": \"CAP-ZEPHYR-DT-SURPRISE-1\", \"citation\": \"ratified citation \\\"quoted\\\"\"}\n  ],\n  \"structurally_absent_slots\": [],\n  \"rejected_slots\": [],\n  \"ignored_match_entries\": []\n}\n"
        );
    }

    /// C02 (spec-convergence-pl011): a carried ignored entry serializes all
    /// four MatchAudit fields, byte-stable.
    #[test]
    fn render_serializes_ignored_match_entries() {
        let mut report = sample();
        report.ignored_match_entries = vec![MatchAudit {
            entry: "devicetree compatible=\"regseq,pl011\"".into(),
            reason: regseq_backends::match_audit::reason_for("amba_driver"),
            license: regseq_backends::match_audit::MATCH_AUDIT_LICENSE,
            native_on: &["zephyr_c"],
        }];
        let a = render_json(&report);
        assert_eq!(a, render_json(&report), "byte-stable across runs");
        assert!(
            a.contains(
                "  \"ignored_match_entries\": [\n    {\"entry\": \"devicetree compatible=\\\"regseq,pl011\\\"\", \"reason\": \"foreign transport: not consumed by the ratified amba_driver registration form; >=1 native entry present\", \"license\": \"spec-convergence-pl011 GRILL D08 (D03(a) rule)\", \"native_on\": [\"zephyr_c\"]}\n  ]\n}\n"
            ),
            "{a}"
        );
    }
}
