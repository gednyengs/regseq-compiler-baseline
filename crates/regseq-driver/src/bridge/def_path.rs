//! Shared rustc -> RegseqModel `DefPathData` mapping.
//!
//! Lifted from `synth/closure.rs` so both closure synthesis and the
//! Phase-2 baseline FnDef-interning path (regseq-driver-46) share one
//! mapper. Pinned against `rustc_hir::definitions::DefPathData` for
//! the `nightly-2026-02-01` toolchain (19 variants per spec.md
//! section 7.1 lines 2020-2056).

use rustc_hir::definitions::DefPathData as RustcDefPathData;

use regseq_model::{DefPathData, StringInterner};

/// Maps a rustc `DefPathData` to the RegseqModel variant. Covers all
/// 19 variants per spec.md section 7.1 (`spec.md:2026-2056`) and the
/// pinned-toolchain `rustc_hir/src/definitions.rs`.
///
/// Takes `&mut StringInterner` (rather than `&mut BridgeCtxt`) so
/// callers can invoke it without holding a mutable borrow on the
/// entire bridge context.
pub(crate) fn translate_def_path_data(
    strings: &mut StringInterner,
    data: RustcDefPathData,
) -> DefPathData {
    match data {
        RustcDefPathData::CrateRoot => DefPathData::CrateRoot,
        RustcDefPathData::Impl => DefPathData::Impl,
        RustcDefPathData::ForeignMod => DefPathData::ForeignMod,
        RustcDefPathData::Use => DefPathData::Use,
        RustcDefPathData::GlobalAsm => DefPathData::GlobalAsm,
        RustcDefPathData::TypeNs(symbol) => DefPathData::TypeNs(strings.intern(symbol.as_str())),
        RustcDefPathData::ValueNs(symbol) => DefPathData::ValueNs(strings.intern(symbol.as_str())),
        RustcDefPathData::MacroNs(symbol) => DefPathData::MacroNs(strings.intern(symbol.as_str())),
        RustcDefPathData::LifetimeNs(symbol) => {
            DefPathData::LifetimeNs(strings.intern(symbol.as_str()))
        }
        RustcDefPathData::Closure => DefPathData::Closure,
        RustcDefPathData::Ctor => DefPathData::Ctor,
        RustcDefPathData::AnonConst => DefPathData::AnonConst,
        RustcDefPathData::LateAnonConst => DefPathData::LateAnonConst,
        RustcDefPathData::DesugaredAnonymousLifetime => DefPathData::DesugaredAnonymousLifetime,
        RustcDefPathData::OpaqueTy => DefPathData::OpaqueTy,
        RustcDefPathData::OpaqueLifetime(symbol) => {
            DefPathData::OpaqueLifetime(strings.intern(symbol.as_str()))
        }
        RustcDefPathData::AnonAssocTy(symbol) => {
            DefPathData::AnonAssocTy(strings.intern(symbol.as_str()))
        }
        RustcDefPathData::SyntheticCoroutineBody => DefPathData::SyntheticCoroutineBody,
        RustcDefPathData::NestedStatic => DefPathData::NestedStatic,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustc_span::{create_default_session_globals_then, Symbol};

    fn name(strings: &StringInterner, data: &DefPathData) -> Option<String> {
        match *data {
            DefPathData::TypeNs(s)
            | DefPathData::ValueNs(s)
            | DefPathData::MacroNs(s)
            | DefPathData::LifetimeNs(s)
            | DefPathData::OpaqueLifetime(s)
            | DefPathData::AnonAssocTy(s) => Some(strings.resolve(s).to_owned()),
            _ => None,
        }
    }

    /// Covers all 19 variants per spec.md section 7.1
    /// (`spec.md:2026-2056`). Constructs one `rustc_hir::definitions::DefPathData`
    /// per variant and asserts the helper returns the matching model
    /// `DefPathData`. Name-bearing variants assert the interned
    /// `StringId` resolves to the source `Symbol`'s string.
    #[test]
    fn translate_def_path_data_covers_all_nineteen_variants() {
        create_default_session_globals_then(|| {
            let mut strings = StringInterner::new();

            let foo = Symbol::intern("foo");
            let bar = Symbol::intern("bar");
            let baz = Symbol::intern("baz");
            let lt = Symbol::intern("'a");
            let olt = Symbol::intern("'b");
            let an = Symbol::intern("Assoc");

            // Anonymous / no-payload variants.
            assert!(matches!(
                translate_def_path_data(&mut strings, RustcDefPathData::CrateRoot),
                DefPathData::CrateRoot
            ));
            assert!(matches!(
                translate_def_path_data(&mut strings, RustcDefPathData::Impl),
                DefPathData::Impl
            ));
            assert!(matches!(
                translate_def_path_data(&mut strings, RustcDefPathData::ForeignMod),
                DefPathData::ForeignMod
            ));
            assert!(matches!(
                translate_def_path_data(&mut strings, RustcDefPathData::Use),
                DefPathData::Use
            ));
            assert!(matches!(
                translate_def_path_data(&mut strings, RustcDefPathData::GlobalAsm),
                DefPathData::GlobalAsm
            ));
            assert!(matches!(
                translate_def_path_data(&mut strings, RustcDefPathData::Closure),
                DefPathData::Closure
            ));
            assert!(matches!(
                translate_def_path_data(&mut strings, RustcDefPathData::Ctor),
                DefPathData::Ctor
            ));
            assert!(matches!(
                translate_def_path_data(&mut strings, RustcDefPathData::AnonConst),
                DefPathData::AnonConst
            ));
            assert!(matches!(
                translate_def_path_data(&mut strings, RustcDefPathData::LateAnonConst),
                DefPathData::LateAnonConst
            ));
            assert!(matches!(
                translate_def_path_data(&mut strings, RustcDefPathData::DesugaredAnonymousLifetime),
                DefPathData::DesugaredAnonymousLifetime
            ));
            assert!(matches!(
                translate_def_path_data(&mut strings, RustcDefPathData::OpaqueTy),
                DefPathData::OpaqueTy
            ));
            assert!(matches!(
                translate_def_path_data(&mut strings, RustcDefPathData::SyntheticCoroutineBody),
                DefPathData::SyntheticCoroutineBody
            ));
            assert!(matches!(
                translate_def_path_data(&mut strings, RustcDefPathData::NestedStatic),
                DefPathData::NestedStatic
            ));

            // Name-bearing variants: assert the interned StringId
            // resolves to the source Symbol's string.
            let type_ns = translate_def_path_data(&mut strings, RustcDefPathData::TypeNs(foo));
            assert!(matches!(type_ns, DefPathData::TypeNs(_)));
            assert_eq!(name(&strings, &type_ns).as_deref(), Some("foo"));

            let value_ns = translate_def_path_data(&mut strings, RustcDefPathData::ValueNs(bar));
            assert!(matches!(value_ns, DefPathData::ValueNs(_)));
            assert_eq!(name(&strings, &value_ns).as_deref(), Some("bar"));

            let macro_ns = translate_def_path_data(&mut strings, RustcDefPathData::MacroNs(baz));
            assert!(matches!(macro_ns, DefPathData::MacroNs(_)));
            assert_eq!(name(&strings, &macro_ns).as_deref(), Some("baz"));

            let lifetime_ns =
                translate_def_path_data(&mut strings, RustcDefPathData::LifetimeNs(lt));
            assert!(matches!(lifetime_ns, DefPathData::LifetimeNs(_)));
            assert_eq!(name(&strings, &lifetime_ns).as_deref(), Some("'a"));

            let opaque_lt =
                translate_def_path_data(&mut strings, RustcDefPathData::OpaqueLifetime(olt));
            assert!(matches!(opaque_lt, DefPathData::OpaqueLifetime(_)));
            assert_eq!(name(&strings, &opaque_lt).as_deref(), Some("'b"));

            let anon_assoc =
                translate_def_path_data(&mut strings, RustcDefPathData::AnonAssocTy(an));
            assert!(matches!(anon_assoc, DefPathData::AnonAssocTy(_)));
            assert_eq!(name(&strings, &anon_assoc).as_deref(), Some("Assoc"));
        });
    }
}
