//! Procedural macros for `async-cell-lock`. Use them through the re-exports
//! `async_cell_lock::deadlock_check` and `async_cell_lock::tokio_test`.

use proc_macro::TokenStream;
use quote::quote;
use syn::{Expr, ItemFn, parse_macro_input, spanned::Spanned};

/// Runs the body of an `async fn` inside a deadlock-check scope.
///
/// Locks from `async-cell-lock` must be used inside a deadlock-check scope, otherwise they
/// return `Error::NotDeadlockCheckFuture`. This attribute rewrites
///
/// ```ignore
/// #[async_cell_lock::deadlock_check]
/// async fn my_fn(arg: Arg) -> Ret { /* body */ }
/// ```
///
/// into
///
/// ```ignore
/// async fn my_fn(arg: Arg) -> Ret {
///     ::async_cell_lock::with_deadlock_check(async move { /* body */ }, "my_fn").await
/// }
/// ```
///
/// Arguments, `?`, early `return` and the return type keep working unchanged.
///
/// # Task name
///
/// The task name (used in deadlock errors and telemetry) defaults to the function name.
/// It can be overridden with any expression implementing `Into<String>`:
///
/// ```ignore
/// #[async_cell_lock::deadlock_check("job::nightly")]
/// async fn nightly_job() {}
/// ```
///
/// # Stacking with runtime attributes
///
/// The attribute composes with `#[tokio::test]`, `#[actix_rt::test]`, `#[tokio::main]`, etc.,
/// but it **must be placed above them**: attributes expand top-down and runtime attributes turn
/// the function into a regular `fn`. Placing it below produces a compile error.
///
/// ```ignore
/// #[async_cell_lock::deadlock_check]
/// #[tokio::test]
/// async fn my_test() -> async_cell_lock::Result<()> {
///     let lock = async_cell_lock::QueueRwLock::new(0, "lock");
///     *lock.queue().await?.write().await? += 1;
///     Ok(())
/// }
/// ```
///
/// For tests, [`macro@tokio_test`] is a shorthand for this combination.
///
/// # Errors
///
/// Compile error if the function is not `async`.
#[proc_macro_attribute]
pub fn deadlock_check(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut f = parse_macro_input!(item as ItemFn);

    let name = if attr.is_empty() {
        None
    } else {
        let e = parse_macro_input!(attr as Expr);
        Some(quote!(#e))
    };

    match wrap(&mut f, name, "#[deadlock_check]") {
        Ok(()) => quote!(#f).into(),
        Err(e) => e.to_compile_error().into(),
    }
}

/// Drop-in replacement for `#[tokio::test]` that runs the test inside a deadlock-check scope.
///
/// ```ignore
/// #[async_cell_lock::tokio_test]
/// async fn my_test() -> async_cell_lock::Result<()> {
///     let lock = async_cell_lock::QueueRwLock::new(0, "lock");
///     *lock.queue().await?.write().await? += 1;
///     Ok(())
/// }
/// ```
///
/// is exactly equivalent to:
///
/// ```ignore
/// #[async_cell_lock::deadlock_check]
/// #[tokio::test]
/// async fn my_test() -> async_cell_lock::Result<()> { /* ... */ }
/// ```
///
/// All arguments are forwarded to `#[tokio::test]`:
///
/// ```ignore
/// #[async_cell_lock::tokio_test(flavor = "multi_thread", worker_threads = 2)]
/// async fn my_multi_thread_test() {}
/// ```
///
/// The task name is the function name; to customize it, use
/// `#[deadlock_check("name")]` above `#[tokio::test]` instead.
///
/// # Requirements
///
/// The expansion references `::tokio::test`, so the calling crate needs `tokio` with the
/// `macros` feature (and `rt-multi-thread` for the `multi_thread` flavor), typically in
/// `[dev-dependencies]`.
///
/// # Errors
///
/// Compile error if the function is not `async`.
#[proc_macro_attribute]
pub fn tokio_test(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mut f = parse_macro_input!(item as ItemFn);
    let args = proc_macro2::TokenStream::from(attr);

    if let Err(e) = wrap(&mut f, None, "#[tokio_test]") {
        return e.to_compile_error().into();
    }

    quote!(
        #[::tokio::test(#args)]
        #f
    )
    .into()
}

fn wrap(
    f: &mut ItemFn,
    name: Option<proc_macro2::TokenStream>,
    attr_name: &str,
) -> syn::Result<()> {
    if f.sig.asyncness.is_none() {
        return Err(syn::Error::new(
            f.sig.fn_token.span(),
            format!(
                "{attr_name} requires an `async fn`; when stacked with a runtime attribute \
                 (e.g. #[tokio::test]), place {attr_name} above it"
            ),
        ));
    }

    let name = name.unwrap_or_else(|| {
        let s = f.sig.ident.to_string();
        quote!(#s)
    });

    let body = &f.block;

    f.block = syn::parse_quote!({
        ::async_cell_lock::with_deadlock_check(async move #body, #name).await
    });

    Ok(())
}
