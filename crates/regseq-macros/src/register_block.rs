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
    /// `name: AccessKind { fields }` — a single register.
    Register(RegisterEntry),
    /// `name[count]: AccessKind { fields }` — `count` identical registers at a
    /// contiguous stride (e.g. E1000 `MTA[128]` / `VFTA[128]`).
    RegisterArray(RegisterArrayEntry),
    /// `name[count]: { sub: AccessKind { fields }, ... }` — `count` identical
    /// register *structs* at a contiguous stride (e.g. E1000 `RA[15]`, each entry
    /// an `RAL`+`RAH` pair, an 8-byte stride).
    StructArray(StructArrayEntry),
    /// `name: Reserved<T> [count]` — a `#[repr(C)]` spacer.
    Reserved(ReservedEntry),
}

struct RegisterEntry {
    name: Ident,
    access: AccessKind,
    fields: Vec<FieldDef>,
}

struct RegisterArrayEntry {
    name: Ident,
    count: u64,
    access: AccessKind,
    fields: Vec<FieldDef>,
}

struct StructArrayEntry {
    name: Ident,
    count: u64,
    subs: Vec<RegisterEntry>,
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

/// Round `offset` up to the next multiple of `align`, matching how `#[repr(C)]`
/// places each field at its alignment. Keeps the macro's running `byte_offset`
/// (and the `OFFSET` cross-check guards) in step with the real layout even for
/// mixed-width blocks (e.g. a `Reserved<u8>` before a 32-bit register).
fn align_up(offset: u64, align: u64) -> u64 {
    offset.div_ceil(align) * align
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
}

/// Generate the per-register field descriptor items (a `Field` const for a plain
/// bitfield, or a sub-module with a `FIELD` const + named `FieldValue`s). The
/// items are module-relative — they reference `Register` / `super::Register`, so
/// they are valid inside any per-register module (single, array element, or a
/// struct-array sub-register). Descriptors carry only register-relative
/// `mask`/`shift`/`value`; the register byte offset comes from the receiver.
fn gen_field_items(fields: &[FieldDef], width_ty: &Ident) -> Vec<TokenStream> {
    fields
        .iter()
        .map(|field| {
            let field_name = &field.name;
            let mask = compute_mask(field.numbits, field.offset);
            let shift = field.offset;

            if field.values.is_empty() {
                // Direct constant (no named values)
                quote! {
                    /// Field descriptor (`mask` + `shift`).
                    pub const #field_name: crate::regseq::Field<#width_ty, Register> =
                        crate::regseq::Field::new(#mask as #width_ty, #shift as #width_ty);
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
                            /// Named field value (`mask` + composed `value`).
                            pub const #nv_name: crate::regseq::FieldValue<#width_ty, super::Register> =
                                crate::regseq::FieldValue::new(#mask as #width_ty, #nv_value as #width_ty);
                        }
                    })
                    .collect();

                quote! {
                    #[allow(non_snake_case)]
                    #[allow(non_upper_case_globals)]
                    pub mod #field_name {
                        /// Field descriptor (`mask` + `shift`).
                        pub const FIELD: crate::regseq::Field<#width_ty, super::Register> =
                            crate::regseq::Field::new(#mask as #width_ty, #shift as #width_ty);
                        #(#named_values)*
                    }
                }
            }
        })
        .collect()
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

/// Parse `AccessKind { fields }` (the tail after a register name + `:`).
fn parse_access_and_fields(input: ParseStream) -> syn::Result<(AccessKind, Vec<FieldDef>)> {
    let kind: Ident = input.parse()?;
    let access = parse_access_kind(&kind)?;
    let brace;
    braced!(brace in input);
    let fields: Vec<FieldDef> = brace
        .parse_terminated(FieldDef::parse, Token![,])?
        .into_iter()
        .collect();
    Ok((access, fields))
}

/// Parse one struct-array sub-register: `name: AccessKind { fields }`.
fn parse_sub_register(input: ParseStream) -> syn::Result<RegisterEntry> {
    let name: Ident = input.parse()?;
    input.parse::<Token![:]>()?;
    let (access, fields) = parse_access_and_fields(input)?;
    Ok(RegisterEntry {
        name,
        access,
        fields,
    })
}

/// Parse a single block entry (single register, register array, struct array, or
/// reserved).
fn parse_block_entry(input: ParseStream) -> syn::Result<BlockEntry> {
    let name: Ident = input.parse()?;

    // `name[count]: ...` — an array form (the `[count]` precedes the `:`).
    if input.peek(syn::token::Bracket) {
        let bracket;
        bracketed!(bracket in input);
        let count = bracket.parse::<LitInt>()?.base10_parse::<u64>()?;
        input.parse::<Token![:]>()?;

        if input.peek(syn::token::Brace) {
            // Struct array: `name[count]: { sub: AccessKind { fields }, ... }`.
            let brace;
            braced!(brace in input);
            let subs: Vec<RegisterEntry> = brace
                .parse_terminated(parse_sub_register, Token![,])?
                .into_iter()
                .collect();
            return Ok(BlockEntry::StructArray(StructArrayEntry {
                name,
                count,
                subs,
            }));
        }
        // Register array: `name[count]: AccessKind { fields }`.
        let (access, fields) = parse_access_and_fields(input)?;
        return Ok(BlockEntry::RegisterArray(RegisterArrayEntry {
            name,
            count,
            access,
            fields,
        }));
    }

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
        let width_bytes = access_width / 8;

        let mut byte_offset: u64 = 0;
        let mut struct_fields = Vec::new();
        // Field initializers for the `Regs::new()` constructor, so an `Mmio<Regs>`
        // can hold the block for `Deref`-based field access. The block instance is
        // a layout carrier only (its storage is never read — every access lowers
        // to a volatile device-model hook).
        let mut field_inits = Vec::new();
        let mut register_modules = Vec::new();

        for entry in &self.entries {
            match entry {
                BlockEntry::Register(reg) => {
                    let reg_name = &reg.name;
                    let wrapper_ty = reg.access.wrapper_ident();
                    byte_offset = align_up(byte_offset, width_bytes);
                    let current_offset = byte_offset as u32;

                    // Struct field (Form B: the register is a typed wrapper field
                    // whose `#[repr(C)]` byte offset the backend recovers from the
                    // block layout — the descriptor carries no register offset).
                    struct_fields.push(quote! {
                        pub #reg_name: crate::regseq::#wrapper_ty<#width_ty, #reg_name::Register>
                    });
                    field_inits.push(quote! {
                        #reg_name: crate::regseq::#wrapper_ty::<#width_ty, #reg_name::Register>::new()
                    });

                    let field_items = gen_field_items(&reg.fields, &width_ty);
                    register_modules.push(quote! {
                        #[allow(non_snake_case)]
                        #[allow(non_upper_case_globals)]
                        pub mod #reg_name {
                            /// Unique marker type for this register.
                            pub enum Register {}

                            /// Byte offset within the block. Not consumed by
                            /// lowering (the backend derives the offset from the
                            /// `#[repr(C)]` layout); kept as an independent
                            /// compile-time guard against a miscounted `Reserved`.
                            pub const OFFSET: u32 = #current_offset;

                            #(#field_items)*
                        }
                    });

                    byte_offset += width_bytes;
                }
                BlockEntry::RegisterArray(arr) => {
                    let arr_name = &arr.name;
                    let wrapper_ty = arr.access.wrapper_ident();
                    byte_offset = align_up(byte_offset, width_bytes);
                    let current_offset = byte_offset as u32;
                    let count = arr.count as usize;

                    // `[Wrapper; count]` — a contiguous register array. Each
                    // element's byte offset (base + i*stride) is recovered by the
                    // backend from the receiver's `Index` projection + layout.
                    struct_fields.push(quote! {
                        pub #arr_name: [crate::regseq::#wrapper_ty<#width_ty, #arr_name::Register>; #count]
                    });
                    field_inits.push(quote! {
                        #arr_name: [crate::regseq::#wrapper_ty::<#width_ty, #arr_name::Register>::new(); #count]
                    });

                    let field_items = gen_field_items(&arr.fields, &width_ty);
                    register_modules.push(quote! {
                        #[allow(non_snake_case)]
                        #[allow(non_upper_case_globals)]
                        pub mod #arr_name {
                            /// Unique marker type for this register array's elements.
                            pub enum Register {}

                            /// Byte offset of the array base within the block.
                            pub const OFFSET: u32 = #current_offset;

                            #(#field_items)*
                        }
                    });

                    byte_offset += arr.count * width_bytes;
                }
                BlockEntry::StructArray(sa) => {
                    let sa_name = &sa.name;
                    // The element struct's alignment is its max sub-register
                    // alignment = the block access width (all subs share it).
                    byte_offset = align_up(byte_offset, width_bytes);
                    let current_offset = byte_offset as u32;
                    let count = sa.count as usize;

                    // The element struct: one typed wrapper field per sub-register,
                    // plus a per-sub-register module (marker + descriptors).
                    let mut entry_fields = Vec::new();
                    let mut entry_inits = Vec::new();
                    let mut sub_modules = Vec::new();
                    for sub in &sa.subs {
                        let sub_name = &sub.name;
                        let sub_wrapper = sub.access.wrapper_ident();
                        entry_fields.push(quote! {
                            pub #sub_name: crate::regseq::#sub_wrapper<#width_ty, #sub_name::Register>
                        });
                        entry_inits.push(quote! {
                            #sub_name: crate::regseq::#sub_wrapper::<#width_ty, #sub_name::Register>::new()
                        });
                        let field_items = gen_field_items(&sub.fields, &width_ty);
                        sub_modules.push(quote! {
                            #[allow(non_snake_case)]
                            #[allow(non_upper_case_globals)]
                            pub mod #sub_name {
                                /// Unique marker type for this sub-register.
                                pub enum Register {}
                                #(#field_items)*
                            }
                        });
                    }
                    let entry_size = sa.subs.len() as u64 * width_bytes;

                    struct_fields.push(quote! {
                        pub #sa_name: [#sa_name::Entry; #count]
                    });
                    field_inits.push(quote! {
                        #sa_name: [#sa_name::Entry::new(); #count]
                    });

                    register_modules.push(quote! {
                        #[allow(non_snake_case)]
                        #[allow(non_upper_case_globals)]
                        pub mod #sa_name {
                            /// One element of the register-struct array (e.g. an
                            /// `RAL`+`RAH` pair). `Copy` so the array initializes as
                            /// `[Entry::new(); N]` (a runtime Repeat, not a const).
                            #[repr(C)]
                            #[derive(Clone, Copy)]
                            pub struct Entry {
                                #(#entry_fields),*
                            }
                            impl Entry {
                                pub fn new() -> Self {
                                    Entry { #(#entry_inits),* }
                                }
                            }

                            /// Byte offset of the array base within the block.
                            pub const OFFSET: u32 = #current_offset;

                            #(#sub_modules)*
                        }
                    });

                    byte_offset += sa.count * entry_size;
                }
                BlockEntry::Reserved(res) => {
                    let res_name = &res.name;
                    let res_width_ty = &res.width_ty;
                    let count = res.count as usize;
                    byte_offset = align_up(byte_offset, type_size_bytes(res_width_ty));

                    struct_fields.push(quote! {
                        #res_name: [crate::regseq::Reserved<#res_width_ty>; #count]
                    });
                    field_inits.push(quote! {
                        #res_name: [crate::regseq::Reserved::<#res_width_ty>::new(); #count]
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

                impl Regs {
                    /// Construct the layout-carrier block instance (default-zeroed;
                    /// never read — every access lowers to a device-model hook).
                    pub fn new() -> Self {
                        Regs {
                            #(#field_inits),*
                        }
                    }
                }

                #(#register_modules)*
            }
        }
    }
}
