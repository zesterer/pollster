#![doc = include_str!("../README.md")]

use std::iter::FromIterator;
use std::str::FromStr;

use proc_macro2::TokenStream;
use quote::ToTokens;
use syn::spanned::Spanned;
use syn::{Error, Expr, ExprLit, ExprPath, ItemFn, Lit, MetaNameValue, Result};

/// Uses [`pollster::block_on`] to enable `async fn main() {}`.
///
/// # Example
///
/// ```
/// #[pollster::main]
/// async fn main() {
///     let my_fut = async {};
///
///     my_fut.await;
/// }
/// ```
#[proc_macro_attribute]
pub fn main(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = TokenStream::from(item);
    let backup = item.clone();

    match common(attr.into(), item) {
        Ok(output) => output.into_token_stream().into(),
        Err(error) => TokenStream::from_iter([error.into_compile_error(), backup]).into(),
    }
}

/// Uses [`pollster::block_on`] to enable `async` on test functions.
///
/// # Example
///
/// ```ignore
/// #[pollster::test]
/// async fn main() {
///     let my_fut = async {};
///
///     my_fut.await;
/// }
/// ```
#[proc_macro_attribute]
pub fn test(
    attr: proc_macro::TokenStream,
    item: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    let item = TokenStream::from(item);
    let backup = item.clone();

    match test_internal(attr.into(), item) {
        Ok(output) => output.into_token_stream().into(),
        Err(error) => TokenStream::from_iter([error.into_compile_error(), backup]).into(),
    }
}

fn test_internal(attr: TokenStream, item: TokenStream) -> Result<ItemFn> {
    let mut item = common(attr, item)?;
    item.attrs.push(syn::parse_quote! { #[test] });

    Ok(item)
}

fn common(attr: TokenStream, item: TokenStream) -> Result<ItemFn> {
    let mut item: ItemFn = syn::parse2(item)?;

    if item.sig.asyncness.is_some() {
        item.sig.asyncness = None;
    } else {
        return Err(Error::new_spanned(item, "expected function to be async"));
    }

    let path = if attr.is_empty() {
        quote::quote! { ::pollster }
    } else {
        let attr: MetaNameValue = syn::parse2(attr)?;

        if attr.path.is_ident("crate") {
            match attr.value {
                Expr::Lit(ExprLit {
                    attrs,
                    lit: Lit::Str(str),
                }) if attrs.is_empty() => TokenStream::from_str(&str.value())?,
                Expr::Path(ExprPath {
                    attrs,
                    qself: None,
                    path,
                }) if attrs.is_empty() => path.to_token_stream(),
                _ => {
                    return Err(Error::new_spanned(
                        attr.value,
                        "expected valid path, e.g. `::package_name`",
                    ))
                }
            }
        } else {
            return Err(Error::new_spanned(attr.path, "expected `crate`"));
        }
    };

    let span = item.span();
    let block = item.block;
    item.block = syn::parse_quote_spanned! {
        span =>
        {
            #path::block_on(async {
                #block
            })
        }
    };

    Ok(item)
}
