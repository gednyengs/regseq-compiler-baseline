use proc_macro2::TokenStream;
use quote::quote;
use syn::parse::{Parse, ParseStream};
use syn::{bracketed, parenthesized, Ident, LitInt, Token};

pub(crate) struct RegisterBitfields {
    width: Ident,
    registers: Vec<RegisterDef>,
}

struct RegisterDef {
    name: Ident,
    fields: Vec<FieldDef>,
}

struct FieldDef {
    name: Ident,
    offset: u64,
    numbits: u64,
    values: Vec<NamedValue>,
}

struct NamedValue {
    name: Ident,
    value: u64,
}

impl Parse for NamedValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        input.parse::<Token![=]>()?;
        let lit: LitInt = input.parse()?;
        let value = lit.base10_parse::<u64>()?;
        Ok(Self { name, value })
    }
}

impl Parse for FieldDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;

        let kw: Ident = input.parse()?;
        if kw != "OFFSET" {
            return Err(syn::Error::new(kw.span(), "expected `OFFSET`"));
        }
        let paren;
        parenthesized!(paren in input);
        let offset = paren.parse::<LitInt>()?.base10_parse::<u64>()?;

        let kw: Ident = input.parse()?;
        if kw != "NUMBITS" {
            return Err(syn::Error::new(kw.span(), "expected `NUMBITS`"));
        }
        let paren;
        parenthesized!(paren in input);
        let numbits = paren.parse::<LitInt>()?.base10_parse::<u64>()?;

        let bracket;
        bracketed!(bracket in input);
        let values: Vec<NamedValue> = if bracket.is_empty() {
            Vec::new()
        } else {
            bracket
                .parse_terminated(NamedValue::parse, Token![,])?
                .into_iter()
                .collect()
        };

        Ok(Self {
            name,
            offset,
            numbits,
            values,
        })
    }
}

impl Parse for RegisterDef {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let name: Ident = input.parse()?;
        let bracket;
        bracketed!(bracket in input);
        let fields: Vec<FieldDef> = bracket
            .parse_terminated(FieldDef::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(Self { name, fields })
    }
}

impl Parse for RegisterBitfields {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let width: Ident = input.parse()?;
        input.parse::<Token![,]>()?;
        let registers: Vec<RegisterDef> = input
            .parse_terminated(RegisterDef::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(Self { width, registers })
    }
}

fn compute_mask(numbits: u64, offset: u64) -> u64 {
    ((1u64 << numbits) - 1) << offset
}

impl RegisterBitfields {
    pub(crate) fn expand(self) -> TokenStream {
        let width = &self.width;

        let register_modules: Vec<TokenStream> = self
            .registers
            .iter()
            .map(|reg| {
                let reg_name = &reg.name;

                let field_modules: Vec<TokenStream> = reg
                    .fields
                    .iter()
                    .map(|field| {
                        let field_name = &field.name;
                        let mask = compute_mask(field.numbits, field.offset);
                        let shift = field.offset;

                        let named_values: Vec<TokenStream> = field
                            .values
                            .iter()
                            .map(|nv| {
                                let nv_name = &nv.name;
                                let nv_value = nv.value << field.offset;
                                quote! {
                                    pub const #nv_name: ::regseq::FieldValue<#width, super::Register> =
                                        ::regseq::FieldValue::new(#mask as #width, #nv_value as #width);
                                }
                            })
                            .collect();

                        quote! {
                            #[allow(non_snake_case)]
                            #[allow(non_upper_case_globals)]
                            pub mod #field_name {
                                pub const FIELD: ::regseq::Field<#width, super::Register> =
                                    ::regseq::Field::new(#mask as #width, #shift as #width);
                                #(#named_values)*
                            }
                        }
                    })
                    .collect();

                quote! {
                    #[allow(non_snake_case)]
                    #[allow(non_upper_case_globals)]
                    pub mod #reg_name {
                        pub struct Register;
                        #(#field_modules)*
                    }
                }
            })
            .collect();

        quote! {
            #(#register_modules)*
        }
    }
}
