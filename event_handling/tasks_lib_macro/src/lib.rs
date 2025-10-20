use proc_macro::TokenStream;
use quote::{format_ident, quote, ToTokens};
use syn::{parse_macro_input, FnArg, ItemFn, PatType, Type};

#[proc_macro_attribute]
pub fn task(_args: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let fn_name = &input_fn.sig.ident;
    let fn_vis = &input_fn.vis;
    let original_fn_signature = &input_fn.sig;
    let original_fn_block = &input_fn.block;
    let attrs = &input_fn.attrs;

    // -- Verify async
    if input_fn.sig.asyncness.is_none() {
        return syn::Error::new_spanned(
            input_fn.sig.fn_token,
            "#[task] can only be applied to async functions",
        )
        .to_compile_error()
        .into();
    }

    // -- Verify input param is (state: SharedState)
    if input_fn.sig.inputs.len() != 1 {
        return syn::Error::new_spanned(
            &input_fn.sig.inputs,
            "#[task] function must take exactly one argument.",
        )
        .to_compile_error()
        .into();
    }
    let first_arg = input_fn.sig.inputs.first().unwrap(); // We checked len >= 1
    let state_param: PatType =
        match first_arg {
            FnArg::Typed(pt) => pt.clone(),
            _ => return syn::Error::new_spanned(
                first_arg,
                "#[task] function argument must be a typed parameter (e.g., `state: SharedState`).",
            )
            .to_compile_error()
            .into(),
        };
    let state_type = &state_param.ty;
    if !matches!(&**state_type, Type::Path(type_path) if type_path.path.segments.last().map_or(false, |s| s.ident == "SharedState"))
    {
        return syn::Error::new_spanned(
            state_type,
            "#[task] function's argument must be `tasks_lib::SharedState`.",
        )
        .to_compile_error()
        .into();
    }

    // -- Verify return type is -> Result<(), Box<dyn Error + Send + Sync>>
    let return_type = &input_fn.sig.output;
    let expected_return_type: syn::ReturnType =
        syn::parse_quote! { -> Result<(), Box<dyn std::error::Error + Send + Sync>> };
    if return_type.into_token_stream().to_string()
        != expected_return_type.to_token_stream().to_string()
    {
        return syn::Error::new_spanned(
            return_type,
            "#[task] function must return `Result<(), Box<dyn std::error::Error + Send + Sync>>`.",
        )
        .to_compile_error()
        .into();
    }

    // TODO name should have full path to ensure uniqueness!
    let task_fn_name = format_ident!("__TASK_{}", fn_name.to_string().to_uppercase());
    let wrapper_fn_name = format_ident!("__TASK_WRAPPER_{}", fn_name.to_string().to_uppercase());

    // Create the wrapped functions
    let expanded = quote! {
        // The original function, kept as-is.
        #(#attrs)*
        #fn_vis #original_fn_signature #original_fn_block

        // A wrapper function that converts the `impl Future` from the original
        // async function into a `Pin<Box<dyn Future>>` to match tasks_lib::EventHandlerTask.
        #[doc(hidden)]
        #fn_vis fn #task_fn_name(state: tasks_lib::SharedState) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), Box<dyn std::error::Error + Send + Sync>>> + Send + 'static>> {
            Box::pin(#fn_name(state))
        }

        // --- Compile-time Registration using linkme ---
        #[linkme::distributed_slice(tasks_lib::TASK_REGISTRY)]
        #[doc(hidden)]
        static #wrapper_fn_name: tasks_lib::EventHandlerTask = #task_fn_name;
    };

    expanded.into()
}
