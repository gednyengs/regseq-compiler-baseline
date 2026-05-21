#![no_std]
#![feature(coroutines, coroutine_trait, stmt_expr_attributes)]
pub fn coroutine_holder() {
    let _c = #[coroutine] || {
        yield 1u32;
    };
}
