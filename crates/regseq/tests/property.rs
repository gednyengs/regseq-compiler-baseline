#![feature(register_tool)]
#![register_tool(rseq)]

//! Compilation tests for the `#[rseq::property(...)]` declaration surface.
//!
//! These tests verify that the accepted firmware-property declaration forms
//! compile as ordinary Rust field attributes. No runtime property-read API
//! or wrapper type is involved; the attribute is purely declarative and
//! compiler-managed.

/// Required field with `id` and `default`.
#[test]
fn property_required_field_with_id_and_default() {
    #[allow(dead_code)]
    struct Driver {
        #[rseq::property(id = "reg-io-width", default = 4)]
        reg_io_width: u32,
    }

    // The field is an ordinary u32 -- no wrapper type required.
    let d = Driver { reg_io_width: 4 };
    assert_eq!(d.reg_io_width, 4);
}

/// Optional field with `id` only.
#[test]
fn property_optional_field_with_id() {
    #[allow(dead_code)]
    struct Driver {
        #[rseq::property(id = "current-speed")]
        current_speed: Option<u32>,
    }

    let d = Driver {
        current_speed: Some(115200),
    };
    assert_eq!(d.current_speed, Some(115200));

    let d_none = Driver {
        current_speed: None,
    };
    assert_eq!(d_none.current_speed, None);
}

/// Field with `vendor_prefix`.
#[test]
fn property_field_with_vendor_prefix() {
    #[allow(dead_code)]
    struct Driver {
        #[rseq::property(id = "periphid", vendor_prefix = "arm")]
        periph_id: Option<u32>,
    }

    let d = Driver {
        periph_id: Some(0x00041011),
    };
    assert_eq!(d.periph_id, Some(0x00041011));
}

/// Required vs optional semantics are purely type-driven: `T` means required,
/// `Option<T>` means optional. Both forms compile under `#[rseq::property(...)]`.
#[test]
fn property_required_vs_optional_type_driven() {
    #[allow(dead_code)]
    struct Driver {
        #[rseq::property(id = "reg-io-width", default = 4)]
        reg_io_width: u32,

        #[rseq::property(id = "current-speed")]
        current_speed: Option<u32>,
    }

    let d = Driver {
        reg_io_width: 4,
        current_speed: None,
    };
    assert_eq!(d.reg_io_width, 4);
    assert_eq!(d.current_speed, None);
}

/// Declared property fields remain ordinary Rust fields. No special wrapper
/// type or import is required to hold or access the value.
#[test]
fn property_fields_are_ordinary_rust_fields() {
    #[allow(dead_code)]
    struct Driver {
        #[rseq::property(id = "reg-io-width", default = 4)]
        reg_io_width: u32,

        #[rseq::property(id = "current-speed")]
        current_speed: Option<u32>,

        #[rseq::property(id = "periphid", vendor_prefix = "arm")]
        periph_id: Option<u32>,
    }

    // Construct with plain field syntax -- no builder, no wrapper.
    let mut d = Driver {
        reg_io_width: 1,
        current_speed: Some(9600),
        periph_id: None,
    };

    // Mutate through ordinary field access.
    d.reg_io_width = 4;
    d.current_speed = Some(115200);
    d.periph_id = Some(0x00041011);

    assert_eq!(d.reg_io_width, 4);
    assert_eq!(d.current_speed, Some(115200));
    assert_eq!(d.periph_id, Some(0x00041011));
}

/// No helper property-read API import is needed. The declarations compile
/// without importing anything beyond ordinary field declarations.
#[test]
fn property_no_helper_api_needed() {
    // This test deliberately avoids `use regseq::*` to prove that the
    // property declaration surface requires zero imports from the facade.

    #[allow(dead_code)]
    struct Driver {
        #[rseq::property(id = "reg-io-width", default = 4)]
        reg_io_width: u32,

        #[rseq::property(id = "current-speed")]
        current_speed: Option<u32>,

        #[rseq::property(id = "periphid", vendor_prefix = "arm")]
        periph_id: Option<u32>,
    }

    let _ = core::mem::size_of::<Driver>();
}
