//! C identifier derivation for the zephyr_c backend.
//!
//! Independent of `generic_c::naming` and `linux_c::naming` (F-1: no shared
//! core); the sanitize rule is the same by necessity (C identifier
//! alphabet), the naming POLICY is Zephyr-native (ratified A-14): the
//! `DriverDescriptor.name` is spent on the log module name and on emitted
//! symbol prefixes (`pl011_init`, `pl011_data_##n`, …) — there is no single
//! Zephyr runtime-name slot (the device name comes from the DT node). The
//! A-8 fallback (TU emission name when no descriptor exists) is dormant in
//! this unit: `emit_zephyr_c` requires exactly one recognized driver.

/// Map an arbitrary token to a C identifier: every byte outside
/// `[A-Za-z0-9_]` becomes `_`; a leading digit gains a `_` prefix; empty
/// input becomes `_`.
pub(crate) fn c_ident(token: &str) -> String {
    let mut out = String::with_capacity(token.len());
    for ch in token.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        out.push('_');
    }
    if out.as_bytes()[0].is_ascii_digit() {
        out.insert(0, '_');
    }
    out
}

/// The `DT_DRV_COMPAT` token for a devicetree compatible string: the DT
/// convention lowercases and maps every non-alphanumeric byte to `_`
/// (`"regseq,pl011"` → `regseq_pl011` — the vendored pin uart_pl011.c:9
/// spells `arm_pl011` for `"arm,pl011"`).
pub(crate) fn compat_token(compatible: &str) -> String {
    c_ident(&compatible.to_ascii_lowercase())
}

/// The last `::` path segment of a rendered def-path (the leaf name the
/// emitted symbol prefix policy applies to).
pub(crate) fn path_leaf(rendered: &str) -> &str {
    rendered.rsplit("::").next().unwrap_or(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_to_c_alphabet() {
        assert_eq!(c_ident("pl011-composed"), "pl011_composed");
        assert_eq!(c_ident("9p"), "_9p");
        assert_eq!(c_ident(""), "_");
    }

    #[test]
    fn compat_token_is_the_dt_convention() {
        assert_eq!(compat_token("regseq,pl011"), "regseq_pl011");
        assert_eq!(compat_token("ARM,PL011"), "arm_pl011");
    }

    #[test]
    fn leaf_takes_the_last_segment() {
        assert_eq!(path_leaf("a::b::on_init"), "on_init");
        assert_eq!(path_leaf("on_init"), "on_init");
    }
}
