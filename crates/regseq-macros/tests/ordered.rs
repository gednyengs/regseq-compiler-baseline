/// Verify `ordered!` is exported from `regseq-macros` and compiles.
#[test]
fn ordered_exported_from_regseq_macros() {
    // The macro is accessible and expands to valid Rust.
    fn _compiles() {
        regseq_macros::ordered! {
            let _x = 1 + 2;
        }
    }
}

/// Verify `ordered!` is re-exported from `regseq` for authored convenience.
#[test]
fn ordered_reexported_from_regseq() {
    fn _compiles() {
        regseq::ordered! {
            let _x = 1 + 2;
        }
    }
}

/// Verify a representative ordered block around register operations compiles.
#[test]
fn ordered_block_compiles_with_register_operations() {
    fn _compiles() {
        regseq::ordered! {
            let a: u32 = 0xFF;
            let b: u32 = a & 0x0F;
            let _c = b | 0x10;
        }
    }
}

/// Verify the macro preserves the final expression value type.
#[test]
fn ordered_preserves_final_expression_value() {
    fn _compiles() -> u32 {
        regseq::ordered! {
            let a = 10u32;
            let b = 20u32;
            a + b
        }
    }
}
