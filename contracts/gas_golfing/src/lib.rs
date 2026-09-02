use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, spanned::Spanned, ItemFn};

#[proc_macro_attribute]
pub fn gas_monitored(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut function = parse_macro_input!(item as ItemFn);

    if function.sig.asyncness.is_some() {
        return syn::Error::new(function.sig.span(), "gas_monitored does not support async functions")
            .to_compile_error()
            .into();
    }

    let fn_name = function.sig.ident.clone();
    let original_block = function.block.clone();

    function.block = syn::parse_quote!({
        let __gas_golfing_start = std::time::Instant::now();
        let __gas_golfing_result = (|| #original_block)();
        let __gas_golfing_elapsed = __gas_golfing_start.elapsed();

        println!(
            "[gas-golfing] {} elapsed={:?}",
            stringify!(#fn_name),
            __gas_golfing_elapsed
        );

        __gas_golfing_result
    });

    quote!(#function).into()
}
