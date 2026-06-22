//! The generic-C per-target host error-mapping table (`generic-c-backend.md`
//! contract C02).
//!
//! A pure, name-keyed function from a `RegseqError` variant name (the
//! `VariantLayout.name` the bridge records) to a libc `<errno.h>` *symbol*.
//! This is the first concrete instance of the per-target errno table the
//! roadmap calls for, and it lives in the backend because the host-error
//! mapping is a per-target concern, not a property of the target-agnostic
//! model.
//!
//! Emission is *symbolic* only (`"EINVAL"`, never a host numeric) so the
//! generated `.c` stays deterministic across hosts (invariant I03); the
//! numerics are resolved at run time by the fixture's own libc. An unknown
//! marked variant maps to `None`, which the adapter caller (regseq-backends-28)
//! turns into a fail-loud emit error -- never a silent fallback to `EIO`
//! (invariant I05, requirement R08). The `Ok(())` -> `0` success case is
//! likewise handled by that caller, not this function.

/// Map a `RegseqError` variant name to its libc `<errno.h>` symbol per the
/// C02 table.
///
/// The mapping is name-keyed and order-independent: two distinct variants
/// may share a symbol (`HardwareError` and `GenericFailure` both map to
/// `"EIO"`). Returns the symbol for each of the nine C02 rows and `None`
/// for any other name -- including a marked variant absent from the table,
/// which the adapter caller must reject as a fail-loud emit error rather
/// than defaulting to a generic code.
///
/// The returned symbol is always a bare `<errno.h>` identifier with no host
/// numeric, preserving the deterministic-`.c` invariant (I03).
// Consumed by the regseq-backends-28 errno adapter
// ([`super::body_emit::emit_exit_adapter`]), which turns a `None` into a
// fail-loud [`super::EmitError::ErrnoMapping`] and emits each `Some` symbol as a
// `return <SYMBOL>;` switch arm.
pub(crate) fn errno_symbol(variant_name: &str) -> Option<&'static str> {
    Some(match variant_name {
        "InvalidParameter" => "EINVAL",
        "OutOfMemory" => "ENOMEM",
        "HardwareError" => "EIO",
        "Timeout" => "ETIMEDOUT",
        "Unsupported" => "EOPNOTSUPP",
        "DeviceBusy" => "EBUSY",
        "PermissionDenied" => "EPERM",
        "DeviceNotFound" => "ENODEV",
        "GenericFailure" => "EIO",
        // Pin-controller-and-GPIO-configuration construct (pinctrl, GRILL D12/C08):
        // the optional `Option<PinCtrl>` absent path compile-time-folds to
        // `Err(RegseqError::PinctrlAbsent)` (D06/C04); the entry-shim errno adapter
        // maps it to `ENODEV` (no such device — the pinctrl resource is absent).
        // generic-c owns this mapping in M3 (where the fold first emits the
        // variant); linux_c/zephyr_c gain the same row in M4 (T19). Each backend
        // fails-loud on an unmapped variant, so the mapping must land WITH the fold.
        "PinctrlAbsent" => "ENODEV",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every variant-name keyed off this list is one C02 row plus its
    /// expected symbol; the test is exhaustive over the table.
    const C02_ROWS: &[(&str, &str)] = &[
        ("InvalidParameter", "EINVAL"),
        ("OutOfMemory", "ENOMEM"),
        ("HardwareError", "EIO"),
        ("Timeout", "ETIMEDOUT"),
        ("Unsupported", "EOPNOTSUPP"),
        ("DeviceBusy", "EBUSY"),
        ("PermissionDenied", "EPERM"),
        ("DeviceNotFound", "ENODEV"),
        ("GenericFailure", "EIO"),
    ];

    #[test]
    fn every_c02_row_maps_to_its_exact_symbol_by_name() {
        for (variant, symbol) in C02_ROWS {
            assert_eq!(
                errno_symbol(variant),
                Some(*symbol),
                "C02 row {variant} must map to {symbol}",
            );
        }
    }

    #[test]
    fn the_two_eio_aliases_both_map_to_eio_order_independent() {
        // HardwareError and GenericFailure intentionally share EIO; the
        // mapping is by name, so their relative order in the table is
        // irrelevant.
        assert_eq!(errno_symbol("HardwareError"), Some("EIO"));
        assert_eq!(errno_symbol("GenericFailure"), Some("EIO"));
    }

    #[test]
    fn symbols_are_bare_errno_identifiers_no_host_numeric() {
        // I03: the emitted symbol is a symbolic <errno.h> name, never a
        // resolved host integer.
        for (_, symbol) in C02_ROWS {
            assert!(
                symbol.starts_with('E') && symbol.bytes().all(|b| b.is_ascii_uppercase()),
                "{symbol} must be a bare uppercase errno symbol, not a numeric",
            );
        }
    }

    #[test]
    fn pinctrl_absent_maps_to_enodev() {
        // GRILL D12/C08: the pinctrl optional-absent fold emits
        // `Err(RegseqError::PinctrlAbsent)`; the errno adapter must map it to
        // ENODEV (else `emit_exit_adapter` fails loud on the unmapped variant when
        // a 3-variant pinctrl RegseqError reaches `regseq_main`'s return). Shares
        // the ENODEV symbol with `DeviceNotFound` (name-keyed, order-independent).
        assert_eq!(errno_symbol("PinctrlAbsent"), Some("ENODEV"));
        assert_eq!(errno_symbol("DeviceNotFound"), Some("ENODEV"));
    }

    #[test]
    fn unknown_variant_name_maps_to_none() {
        // R08 / I05: an unmapped (or bogus) name yields None so the caller
        // can fail loud rather than defaulting to EIO.
        assert_eq!(errno_symbol("Frobnicate"), None);
        assert_eq!(errno_symbol(""), None);
        assert_eq!(errno_symbol("eio"), None);
    }
}
