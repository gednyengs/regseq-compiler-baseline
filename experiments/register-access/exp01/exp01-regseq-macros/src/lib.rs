use proc_macro::TokenStream;
use proc_macro2::TokenTree;
use quote::{format_ident, quote, ToTokens};

#[proc_macro]
pub fn register_bitfields(input: TokenStream) -> TokenStream {
    let tokens: Vec<TokenTree> = proc_macro2::TokenStream::from(input).into_iter().collect();

    let mut i = 0;
    let width = match tokens.get(i) {
        Some(TokenTree::Ident(id)) => {
            i += 1;
            id.clone()
        }
        _ => panic!("expected width type as first token"),
    };

    match tokens.get(i) {
        Some(TokenTree::Punct(p)) if p.as_char() == ',' => i += 1,
        _ => panic!("expected comma after width type"),
    }

    let mut registers = Vec::new();
    while i < tokens.len() {
        if matches!(tokens.get(i), Some(TokenTree::Punct(p)) if p.as_char() == ',') {
            i += 1;
            continue;
        }

        let reg_name = match tokens.get(i) {
            Some(TokenTree::Ident(id)) => {
                i += 1;
                id.to_string()
            }
            None => break,
            _ => panic!("expected register name"),
        };

        let fields_group = match tokens.get(i) {
            Some(TokenTree::Group(g)) if g.delimiter() == proc_macro2::Delimiter::Bracket => {
                i += 1;
                g.clone()
            }
            _ => panic!("expected bracketed field list"),
        };

        registers.push((reg_name, parse_fields(fields_group)));
    }

    let width_ty: syn::Type = syn::parse2(width.into_token_stream()).expect("valid width type");

    let mut reg_modules = Vec::new();
    for (reg_name, fields) in registers {
        let reg_ident = format_ident!("{}", reg_name);
        let mut field_modules = Vec::new();

        for field in fields {
            let field_ident = format_ident!("{}", field.name);
            let offset = field.offset;
            let width = field.width;

            let mut named_values = Vec::new();
            for (name, value) in field.named_values {
                let value_ident = format_ident!("{}", name);
                named_values.push(quote! {
                    pub const #value_ident: ::exp01_regseq::FieldValue<#width_ty, super::Register> =
                        ::exp01_regseq::FieldValue::new(#offset, #width, #value);
                });
            }

            field_modules.push(quote! {
                pub mod #field_ident {
                    pub const FIELD: ::exp01_regseq::Field<#width_ty, super::Register> =
                        ::exp01_regseq::Field::new(#offset, #width);
                    #(#named_values)*
                }
            });
        }

        reg_modules.push(quote! {
            pub mod #reg_ident {
                #[derive(Debug, Clone, Copy, PartialEq, Eq)]
                pub struct Register;
                #(#field_modules)*
            }
        });
    }

    quote! {
        #(#reg_modules)*
    }
    .into()
}

struct ParsedField {
    name: String,
    offset: u8,
    width: u8,
    named_values: Vec<(String, u32)>,
}

fn parse_fields(group: proc_macro2::Group) -> Vec<ParsedField> {
    let tokens: Vec<TokenTree> = group.stream().into_iter().collect();
    let mut i = 0;
    let mut fields = Vec::new();

    while i < tokens.len() {
        if matches!(tokens.get(i), Some(TokenTree::Punct(p)) if p.as_char() == ',') {
            i += 1;
            continue;
        }

        let field_name = match tokens.get(i) {
            Some(TokenTree::Ident(id)) => {
                i += 1;
                id.to_string()
            }
            None => break,
            _ => panic!("expected field name"),
        };

        let offset = parse_kw_call(&tokens, &mut i, "OFFSET");
        let width = parse_kw_call(&tokens, &mut i, "NUMBITS");

        let named_values = match tokens.get(i) {
            Some(TokenTree::Group(g)) if g.delimiter() == proc_macro2::Delimiter::Bracket => {
                i += 1;
                parse_named_values(g.clone())
            }
            _ => panic!("expected bracketed named-value list"),
        };

        fields.push(ParsedField {
            name: field_name,
            offset,
            width,
            named_values,
        });
    }

    fields
}

fn parse_kw_call(tokens: &[TokenTree], i: &mut usize, expected: &str) -> u8 {
    match tokens.get(*i) {
        Some(TokenTree::Ident(id)) if id == expected => *i += 1,
        _ => panic!("expected {}", expected),
    }

    let group = match tokens.get(*i) {
        Some(TokenTree::Group(g)) if g.delimiter() == proc_macro2::Delimiter::Parenthesis => {
            *i += 1;
            g.clone()
        }
        _ => panic!("expected {}(...)", expected),
    };

    let inner: Vec<TokenTree> = group.stream().into_iter().collect();
    match inner.as_slice() {
        [TokenTree::Literal(lit)] => lit.to_string().parse::<u8>().expect("u8 literal"),
        _ => panic!("expected numeric literal in {}", expected),
    }
}

fn parse_named_values(group: proc_macro2::Group) -> Vec<(String, u32)> {
    let tokens: Vec<TokenTree> = group.stream().into_iter().collect();
    let mut i = 0;
    let mut values = Vec::new();

    while i < tokens.len() {
        if matches!(tokens.get(i), Some(TokenTree::Punct(p)) if p.as_char() == ',') {
            i += 1;
            continue;
        }

        let name = match tokens.get(i) {
            Some(TokenTree::Ident(id)) => {
                i += 1;
                id.to_string()
            }
            None => break,
            _ => panic!("expected named value ident"),
        };

        match tokens.get(i) {
            Some(TokenTree::Punct(p)) if p.as_char() == '=' => i += 1,
            _ => panic!("expected '=' in named value"),
        }

        let value = match tokens.get(i) {
            Some(TokenTree::Literal(lit)) => {
                i += 1;
                lit.to_string().parse::<u32>().expect("u32 literal")
            }
            _ => panic!("expected numeric literal in named value"),
        };

        values.push((name, value));
    }

    values
}
