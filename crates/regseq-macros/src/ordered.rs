use proc_macro2::TokenStream;
use quote::quote;

/// Expand `ordered! { ... }` into ordered region markers bracketing
/// the user block.
///
/// The expansion uses `ordered_region_begin()`/`ordered_region_end()`
/// marker functions that the compiler recognizes during IR construction
/// for OrderedRegion detection.
pub fn expand(input: TokenStream) -> TokenStream {
    quote! {
        {
            crate::regseq::internal::ordered_region_begin();
            let __regseq_ordered_result = { #input };
            crate::regseq::internal::ordered_region_end();
            __regseq_ordered_result
        }
    }
}
