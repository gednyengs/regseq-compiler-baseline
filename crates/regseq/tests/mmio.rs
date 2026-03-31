use regseq::Mmio;

#[test]
fn mmio_is_publicly_exported() {
    let _m: Mmio<u32> = Mmio::new(0u32);
}

#[test]
fn mmio_as_driver_struct_field() {
    struct MyRegs {
        pub status: u32,
        pub control: u32,
    }

    struct MyDriver {
        regs: Mmio<MyRegs>,
    }

    let driver = MyDriver {
        regs: Mmio::new(MyRegs {
            status: 0,
            control: 0,
        }),
    };

    // Access through Deref to exercise the fields
    assert_eq!(driver.regs.status, 0);
    assert_eq!(driver.regs.control, 0);
}

#[test]
fn mmio_deref_provides_field_access() {
    struct Pl011Registers {
        dr: u32,
        fr: u32,
    }

    let regs: Mmio<Pl011Registers> = Mmio::new(Pl011Registers { dr: 0xAB, fr: 0xCD });

    // Deref allows direct field access through Mmio<T>
    assert_eq!(regs.dr, 0xAB);
    assert_eq!(regs.fr, 0xCD);
}

#[test]
fn mmio_is_opaque_no_runtime_support_needed() {
    // Mmio<T> can be constructed and used without any backend/runtime setup.
    // This confirms the type is a pure facade shell.
    let m: Mmio<[u8; 4]> = Mmio::new([1, 2, 3, 4]);
    assert_eq!(m[0], 1);
    assert_eq!(m[3], 4);

    // Debug is available
    let _ = format!("{:?}", Mmio::new(42u32));
}
