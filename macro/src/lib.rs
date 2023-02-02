#![doc = include_str!("../README.md")]

use std::iter::FromIterator;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::{Error, ItemFn, Result};

#[proc_macro_attribute]
pub fn main(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    relay(item, internal)
}

#[proc_macro_attribute]
pub fn test(
    _attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    relay(item, |item| {
        let mut item = internal(item)?;
        item.attrs.push(syn::parse_quote! { #[test] });

        Ok(item)
    })
}

fn relay(
    item: proc_macro::TokenStream,
    fun: impl FnOnce(TokenStream) -> Result<ItemFn>,
) -> proc_macro::TokenStream {
    let item = TokenStream::from(item);
    let backup = item.clone();

    match fun(item) {
        Ok(output) => output.into_token_stream().into(),
        Err(error) => TokenStream::from_iter([error.into_compile_error(), backup]).into(),
    }
}

fn internal(item: TokenStream) -> Result<ItemFn> {
    let mut item: ItemFn = syn::parse2(item)?;
    let span = item.block.brace_token.span;

    if item.sig.asyncness.is_some() {
        item.sig.asyncness = None;
    } else {
        return Err(Error::new_spanned(item, "expected function to be async"));
    }

    let block = item.block;
    item.block = syn::parse_quote_spanned! {
        span =>
        {
            ::pollster::block_on(async {
                #block
            })
        }
    };

    Ok(item)
}
