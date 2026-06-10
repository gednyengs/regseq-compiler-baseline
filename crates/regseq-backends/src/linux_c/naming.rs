//! C identifier derivation for the linux_c backend.
//!
//! Independent of `generic_c::naming` (F-1: no shared core); the sanitize
//! rule is the same by necessity (C identifier alphabet), the naming POLICY
//! is Linux-native: every emitted symbol is prefixed with the driver name so
//! the module reads like a hand-written kernel driver (`pl011_probe`,
//! `pl011_on_init`, …).

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

/// The last `::` path segment of a rendered def-path (the leaf name a
/// kernel driver would use for a function).
pub(crate) fn path_leaf(rendered: &str) -> &str {
    rendered.rsplit("::").next().unwrap_or(rendered)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitizes_to_c_alphabet() {
        assert_eq!(c_ident("pl011-composed"), "pl011_composed");
        assert_eq!(c_ident("arm,pl011"), "arm_pl011");
        assert_eq!(c_ident("9p"), "_9p");
        assert_eq!(c_ident(""), "_");
    }

    #[test]
    fn leaf_takes_the_last_segment() {
        assert_eq!(path_leaf("a::b::on_init"), "on_init");
        assert_eq!(path_leaf("on_init"), "on_init");
    }
}
