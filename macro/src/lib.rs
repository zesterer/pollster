#![doc = include_str!("../README.md")]

use std::iter::FromIterator;
use std::str::FromStr;

use proc_macro2::TokenStream;
use quote::{quote_spanned, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::{AttributeArgs, Error, ItemFn, Lit, Meta, MetaNameValue, NestedMeta, Result};

#[proc_macro_attribute]
pub fn main(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = item;
    let item = TokenStream::from(item);
    let mut backup = item.clone();

    match common(&mut backup, attr.into(), item) {
        Ok(output) => output.into_token_stream().into(),
        Err(error) => TokenStream::from_iter([error.into_compile_error(), backup]).into(),
    }
}

#[proc_macro_attribute]
pub fn test(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = item;
    let item = TokenStream::from(item);
    let mut backup = item.clone();

    match test_internal(&mut backup, attr.into(), item) {
        Ok(output) => output.into_token_stream().into(),
        Err(error) => TokenStream::from_iter([error.into_compile_error(), backup]).into(),
    }
}

fn test_internal(backup: &mut TokenStream, attr: TokenStream, item: TokenStream) -> Result<ItemFn> {
    let mut item = common(backup, attr, item)?;
    item.attrs.push(syn::parse_quote! { #[test] });

    Ok(item)
}

fn common(backup: &mut TokenStream, attr: TokenStream, item: TokenStream) -> Result<ItemFn> {
    let mut item: ItemFn = syn::parse2(item)?;
    let span = item.block.brace_token.span;

    if item.sig.asyncness.is_some() {
        item.sig.asyncness = None;
        *backup = item.to_token_stream();
    } else {
        return Err(Error::new_spanned(item, "expected function to be async"));
    }

    let attr: Attr = syn::parse2(attr)?;
    let mut crate_name = None;

    for meta in attr.0 {
        if let NestedMeta::Meta(Meta::NameValue(MetaNameValue {
            path,
            lit: Lit::Str(name),
            ..
        })) = meta
        {
            if path.is_ident("crate") {
                let span = name.span();
                let name = TokenStream::from_str(&name.value())?;

                if crate_name
                    .replace(quote_spanned! { span => #name})
                    .is_some()
                {
                    return Err(Error::new(span, "found duplicate crate attribute"));
                }

                continue;
            }
        }

        return Err(Error::new_spanned(item, "main(crate = \"package-name\")"));
    }

    let crate_name = crate_name.unwrap_or_else(|| quote::quote! { ::pollster });

    let block = item.block;
    item.block = syn::parse_quote_spanned! {
        span =>
        {
            #crate_name::block_on(async {
                #block
            })
        }
    };

    Ok(item)
}

struct Attr(AttributeArgs);

impl Parse for Attr {
    fn parse(input: ParseStream) -> Result<Self> {
        let mut attr = Vec::new();

        while let Ok(meta) = input.parse() {
            attr.push(meta)
        }

        Ok(Self(attr))
    }
}
