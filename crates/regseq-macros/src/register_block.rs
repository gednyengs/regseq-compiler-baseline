use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::{braced, bracketed, parenthesized, Ident, LitInt, LitStr, Token, Visibility};

// ---------------------------------------------------------------------------
// AST types
// ---------------------------------------------------------------------------

pub(crate) struct RegisterBlock {
    endian: String,
    access_width: u64,
    vis: Visibility,
    name: Ident,
    entries: Vec<BlockEntry>,
}

enum BlockEntry {
    Register(RegisterEntry),
    Reserved(ReservedEntry),
}

struct RegisterEntry {
    name: Ident,
    access: AccessKind,
    fields: Vec<FieldDef>,
}

#[derive(Clone, Copy)]
enum AccessKind {
    ReadOnly,
    WriteOnly,
    ReadWrite,
}

struct ReservedEntry {
    name: Ident,
    width_ty: Ident,
    count: u64,
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

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn compute_mask(numbits: u64, offset: u64) -> u64 {
    ((1u64 << numbits) - 1) << offset
}

fn width_type(access_width: u64) -> Ident {
    match access_width {
        8 => format_ident!("u8"),
        16 => format_ident!("u16"),
        32 => format_ident!("u32"),
        other => panic!("unsupported access_width: {other}"),
    }
}

fn type_size_bytes(ty: &Ident) -> u64 {
    match ty.to_string().as_str() {
        "u8" => 1,
        "u16" => 2,
        "u32" => 4,
        other => panic!("unsupported reserved type: {other}"),
    }
}

impl AccessKind {
    fn wrapper_ident(self) -> Ident {
        match self {
            Self::ReadOnly => format_ident!("ReadOnly"),
            Self::WriteOnly => format_ident!("WriteOnly"),
            Self::ReadWrite => format_ident!("ReadWrite"),
        }
    }

    fn access_ident(self) -> Ident {
        match self {
            Self::ReadOnly => format_ident!("ReadOnlyAccess"),
            Self::WriteOnly => format_ident!("WriteOnlyAccess"),
            Self::ReadWrite => format_ident!("ReadWriteAccess"),
        }
    }
}

// ---------------------------------------------------------------------------
// Parsing
// ---------------------------------------------------------------------------

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

fn parse_access_kind(ident: &Ident) -> syn::Result<AccessKind> {
    match ident.to_string().as_str() {
        "ReadOnly" => Ok(AccessKind::ReadOnly),
        "WriteOnly" => Ok(AccessKind::WriteOnly),
        "ReadWrite" => Ok(AccessKind::ReadWrite),
        _ => Err(syn::Error::new(
            ident.span(),
            "expected `ReadOnly`, `WriteOnly`, or `ReadWrite`",
        )),
    }
}

/// Parse a single block entry (register or reserved).
fn parse_block_entry(input: ParseStream) -> syn::Result<BlockEntry> {
    let name: Ident = input.parse()?;
    input.parse::<Token![:]>()?;

    // Peek at the next identifier to decide register vs reserved.
    let kind: Ident = input.parse()?;
    if kind == "Reserved" {
        // Reserved<T> [count]
        input.parse::<Token![<]>()?;
        let width_ty: Ident = input.parse()?;
        input.parse::<Token![>]>()?;
        let bracket;
        bracketed!(bracket in input);
        let count = bracket.parse::<LitInt>()?.base10_parse::<u64>()?;
        Ok(BlockEntry::Reserved(ReservedEntry {
            name,
            width_ty,
            count,
        }))
    } else {
        // AccessKind { fields... }
        let access = parse_access_kind(&kind)?;
        let brace;
        braced!(brace in input);
        let fields: Vec<FieldDef> = brace
            .parse_terminated(FieldDef::parse, Token![,])?
            .into_iter()
            .collect();
        Ok(BlockEntry::Register(RegisterEntry {
            name,
            access,
            fields,
        }))
    }
}

impl Parse for RegisterBlock {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        // Parse optional #[endian = "...", access_width = N]
        let mut endian = String::from("little");
        let mut access_width: u64 = 32;

        if input.peek(Token![#]) {
            input.parse::<Token![#]>()?;
            let bracket;
            bracketed!(bracket in input);
            while !bracket.is_empty() {
                let key: Ident = bracket.parse()?;
                bracket.parse::<Token![=]>()?;
                if key == "endian" {
                    let lit: LitStr = bracket.parse()?;
                    endian = lit.value();
                } else if key == "access_width" {
                    let lit: LitInt = bracket.parse()?;
                    access_width = lit.base10_parse()?;
                } else {
                    return Err(syn::Error::new(
                        key.span(),
                        "expected `endian` or `access_width`",
                    ));
                }
                if !bracket.is_empty() {
                    bracket.parse::<Token![,]>()?;
                }
            }
        }

        // Parse visibility and block name
        let vis: Visibility = input.parse()?;
        let name: Ident = input.parse()?;

        // Parse braced entries
        let brace;
        braced!(brace in input);
        let entries: Vec<BlockEntry> = brace
            .parse_terminated(parse_block_entry, Token![,])?
            .into_iter()
            .collect();

        Ok(Self {
            endian,
            access_width,
            vis,
            name,
            entries,
        })
    }
}

// ---------------------------------------------------------------------------
// Code generation
// ---------------------------------------------------------------------------

impl RegisterBlock {
    pub(crate) fn expand(self) -> TokenStream {
        let vis = &self.vis;
        let block_name = &self.name;
        let endian = &self.endian;
        let access_width = self.access_width;
        let width_ty = width_type(access_width);

        let mut byte_offset: u64 = 0;
        let mut struct_fields = Vec::new();
        let mut register_modules = Vec::new();

        for entry in &self.entries {
            match entry {
                BlockEntry::Register(reg) => {
                    let reg_name = &reg.name;
                    let wrapper_ty = reg.access.wrapper_ident();
                    let access_ty = reg.access.access_ident();
                    let current_offset = byte_offset as u32;

                    // Struct field
                    struct_fields.push(quote! {
                        pub #reg_name: ::regseq::#wrapper_ty<#width_ty, #reg_name::Register>
                    });

                    // Generate field items (constants or sub-modules)
                    let field_items: Vec<TokenStream> = reg
                        .fields
                        .iter()
                        .map(|field| {
                            let field_name = &field.name;
                            let mask = compute_mask(field.numbits, field.offset);
                            let shift = field.offset;

                            if field.values.is_empty() {
                                // Direct constant (no named values)
                                quote! {
                                    /// Field descriptor.
                                    pub const #field_name: ::regseq::Field<#width_ty, Register> =
                                        ::regseq::Field::new(#mask as #width_ty, #shift as #width_ty);
                                }
                            } else {
                                // Sub-module with FIELD constant and named values
                                let named_values: Vec<TokenStream> = field
                                    .values
                                    .iter()
                                    .map(|nv| {
                                        let nv_name = &nv.name;
                                        let nv_value = nv.value << field.offset;
                                        quote! {
                                            /// Named field value.
                                            pub const #nv_name: ::regseq::FieldValue<#width_ty, super::Register> =
                                                ::regseq::FieldValue::new(#mask as #width_ty, #nv_value as #width_ty);
                                        }
                                    })
                                    .collect();

                                quote! {
                                    #[allow(non_snake_case)]
                                    #[allow(non_upper_case_globals)]
                                    pub mod #field_name {
                                        /// Field descriptor.
                                        pub const FIELD: ::regseq::Field<#width_ty, super::Register> =
                                            ::regseq::Field::new(#mask as #width_ty, #shift as #width_ty);
                                        #(#named_values)*
                                    }
                                }
                            }
                        })
                        .collect();

                    // Register module
                    register_modules.push(quote! {
                        #[allow(non_snake_case)]
                        #[allow(non_upper_case_globals)]
                        pub mod #reg_name {
                            /// Unique marker type for this register.
                            pub enum Register {}

                            /// Byte offset within the block.
                            pub const OFFSET: u32 = #current_offset;

                            /// RegisterId for full-register operations.
                            pub const REG: ::regseq::RegisterId<#width_ty, Register> =
                                ::regseq::RegisterId::new();

                            #(#field_items)*

                            impl ::regseq::RegisterOf<super::Regs> for Register {
                                type Access = ::regseq::#access_ty;
                            }
                        }
                    });

                    byte_offset += access_width / 8;
                }
                BlockEntry::Reserved(res) => {
                    let res_name = &res.name;
                    let res_width_ty = &res.width_ty;
                    let count = res.count as usize;

                    struct_fields.push(quote! {
                        #res_name: [::regseq::Reserved<#res_width_ty>; #count]
                    });

                    byte_offset += res.count * type_size_bytes(&res.width_ty);
                }
            }
        }

        let access_width_lit = LitInt::new(&access_width.to_string(), block_name.span());

        quote! {
            #[allow(non_snake_case)]
            #[allow(non_upper_case_globals)]
            #vis mod #block_name {
                #[repr(C)]
                #[rseq::register_block(endian = #endian, access_width = #access_width_lit)]
                pub struct Regs {
                    #(#struct_fields),*
                }

                #(#register_modules)*
            }
        }
    }
}
