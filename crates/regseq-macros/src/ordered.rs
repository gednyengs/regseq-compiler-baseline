use proc_macro2::TokenStream;
use quote::quote;

/// Expand `ordered! { ... }` into a block bracketed by canonical barrier
/// intrinsic calls.
pub fn expand(input: TokenStream) -> TokenStream {
    quote! {
        {
            ::regseq::intrinsics::sync::memory_barrier(
                ::regseq::intrinsics::sync::BarrierKind::Full,
            );
            let __regseq_ordered_result = { #input };
            ::regseq::intrinsics::sync::memory_barrier(
                ::regseq::intrinsics::sync::BarrierKind::Full,
            );
            __regseq_ordered_result
        }
    }
}
