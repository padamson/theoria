//! Proc-macros for theoria.
//!
//! `#[story]` turns a function whose typed parameters are the editable "args"
//! into a `theoria::Story` carrying those args (as live controls), the captured
//! body source (for "show code"), and the doc-comment description (for autodocs).
//! It is theoria's stand-in for Storybook's CSF + react-docgen, which Rust can't
//! provide via runtime reflection.
//!
//! ```ignore
//! #[story(name = "Canvas/Yard", yard_w = 70.0, yard_d = 30.0)]
//! /// The yard canvas, drawn to scale.
//! fn yard(yard_w: f64, yard_d: f64) -> impl IntoView {
//!     view! { <Yard yard_w=yard_w yard_d=yard_d px_ft=12.0 pad=40.0 /> }
//! }
//! ```
//!
//! A parameter's type picks its control through `theoria::StoryArg`; the
//! macro emits the same code for every type.

use proc_macro::TokenStream;
use proc_macro2::{Delimiter, TokenStream as TokenStream2, TokenTree};
use quote::{format_ident, quote, quote_spanned};
use std::collections::HashMap;
use syn::{
    Expr, FnArg, ItemFn, Lit, Meta, Pat, Token, Type, parse::Parser, punctuated::Punctuated,
    spanned::Spanned,
};

#[proc_macro_attribute]
pub fn story(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Thin adapter to the testable `proc_macro2` core; all logic lives in
    // `story_impl`.
    story_impl(attr.into(), item.into()).into()
}

fn story_impl(attr: TokenStream2, item: TokenStream2) -> TokenStream2 {
    let func: ItemFn = match syn::parse2(item) {
        Ok(func) => func,
        Err(e) => return e.to_compile_error(),
    };
    let overrides = match Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attr) {
        Ok(overrides) => overrides,
        Err(e) => return e.to_compile_error(),
    };

    // `name = "..."` sets the story name; any other `key = expr` is a default for
    // the matching arg.
    let mut name_override: Option<Expr> = None;
    let mut defaults: HashMap<String, Expr> = HashMap::new();
    for meta in overrides {
        if let Meta::NameValue(nv) = meta {
            let key = nv
                .path
                .get_ident()
                .map(ToString::to_string)
                .unwrap_or_default();
            if key == "name" {
                name_override = Some(nv.value);
            } else {
                defaults.insert(key, nv.value);
            }
        }
    }

    let fname = func.sig.ident.clone();
    let story_name: Expr = name_override.unwrap_or_else(|| {
        let s = fname.to_string();
        syn::parse_quote!(#s)
    });
    let description = doc_description(&func);

    let args = match func
        .sig
        .inputs
        .iter()
        .map(|input| story_arg(input, &defaults))
        .collect::<syn::Result<Vec<_>>>()
    {
        Ok(args) => args,
        Err(e) => return e.to_compile_error(),
    };
    let sig_lets = args.iter().map(|a| &a.sig_let);
    let get_lets = args.iter().map(|a| &a.get_let);
    let arg_entries = args.iter().map(|a| &a.entry);

    let body = &func.block;
    let source = body_source(&func);

    quote! {
        pub fn #fname() -> theoria::Story {
            #[allow(unused_imports)]
            use leptos::prelude::*;
            #(#sig_lets)*
            let __view = move || {
                #(#get_lets)*
                leptos::prelude::IntoAny::into_any(#body)
            };
            theoria::Story::__from_macro(
                #story_name,
                __view,
                ::std::vec![#(#arg_entries),*],
                #source,
                #description,
            )
        }
    }
}

/// What one story parameter adds to the generated constructor.
struct ArgTokens {
    /// Outside the view closure: the arg's signal and its read side.
    sig_let: TokenStream2,
    /// Inside it: the parameter.
    get_let: TokenStream2,
    /// The `theoria::Arg` for the controls panel.
    entry: TokenStream2,
}

fn story_arg(input: &FnArg, defaults: &HashMap<String, Expr>) -> syn::Result<ArgTokens> {
    let FnArg::Typed(pt) = input else {
        return Err(syn::Error::new_spanned(
            input,
            "#[story] functions take no self",
        ));
    };
    let Pat::Ident(pi) = &*pt.pat else {
        return Err(syn::Error::new_spanned(
            &pt.pat,
            "#[story] args must be plain identifiers",
        ));
    };
    let ident = &pi.ident;
    let mutability = &pi.mutability;
    let name = ident.to_string();
    let sig_ident = format_ident!("__sig_{}", ident);
    let read_ident = format_ident!("__read_{}", ident);
    let ty = &pt.ty;
    let label = type_label(ty);
    // Every use of the type is spanned on it, so a type with no `StoryArg`
    // impl is reported at the parameter, not at the attribute.
    let span = ty.span();
    let arg = quote_spanned!(span=> <#ty as theoria::StoryArg>);
    let default = defaults.get(&name).map_or_else(
        || quote_spanned!(span=> <#arg::Value as ::core::default::Default>::default()),
        |given| {
            quote_spanned!(span=>
                <#arg::Init as ::core::convert::Into<#arg::Value>>::into(#given)
            )
        },
    );
    Ok(ArgTokens {
        sig_let: quote_spanned! {span=>
            let #sig_ident = leptos::prelude::RwSignal::new(#default);
            let #read_ident: leptos::prelude::Signal<#arg::Value> =
                leptos::prelude::Signal::from(#sig_ident);
        },
        get_let: quote_spanned!(span=> let #mutability #ident: #ty = #arg::bind(#read_ident);),
        entry: quote_spanned!(span=>
            theoria::Arg::new(#name, #label, #arg::control(#sig_ident))
        ),
    })
}

/// The type as the story wrote it, shown in the controls panel.
fn type_label(ty: &Type) -> String {
    join_tokens(quote!(#ty))
}

fn join_tokens(tokens: TokenStream2) -> String {
    let mut out = String::new();
    let mut after_word = false;
    for token in tokens {
        match token {
            TokenTree::Group(group) => {
                let (open, close) = match group.delimiter() {
                    Delimiter::Parenthesis => ("(", ")"),
                    Delimiter::Bracket => ("[", "]"),
                    Delimiter::Brace => ("{", "}"),
                    Delimiter::None => ("", ""),
                };
                out.push_str(open);
                out.push_str(&join_tokens(group.stream()));
                out.push_str(close);
                after_word = false;
            }
            TokenTree::Punct(punct) => {
                out.push(punct.as_char());
                if matches!(punct.as_char(), ',' | ';') {
                    out.push(' ');
                }
                after_word = false;
            }
            word @ (TokenTree::Ident(_) | TokenTree::Literal(_)) => {
                if after_word {
                    out.push(' ');
                }
                out.push_str(&word.to_string());
                after_word = true;
            }
        }
    }
    out
}

/// The doc-comment lines joined into a `Some("...")` / `None` expression.
fn doc_description(func: &ItemFn) -> Expr {
    let mut doc = String::new();
    for a in &func.attrs {
        if a.path().is_ident("doc")
            && let Meta::NameValue(nv) = &a.meta
            && let Expr::Lit(syn::ExprLit {
                lit: Lit::Str(s), ..
            }) = &nv.value
        {
            if !doc.is_empty() {
                doc.push('\n');
            }
            doc.push_str(s.value().trim());
        }
    }
    if doc.is_empty() {
        syn::parse_quote!(::std::option::Option::None)
    } else {
        syn::parse_quote!(::std::option::Option::Some(#doc))
    }
}

/// The body's source text (braces stripped) as a string literal, for "show
/// code". Falls back to the token stream when span source text is unavailable.
fn body_source(func: &ItemFn) -> Expr {
    let raw = func
        .block
        .span()
        .source_text()
        .unwrap_or_else(|| quote!(#func).to_string());
    let s = raw
        .trim()
        .trim_start_matches('{')
        .trim_end_matches('}')
        .trim()
        .to_string();
    syn::parse_quote!(#s)
}

#[cfg(test)]
mod tests {
    use super::*;
    use quote::quote;

    /// Pull the string out of a `doc_description` result: `Option::Some("lit")`
    /// → `Some(lit)`, the `Option::None` path → `None`.
    fn some_str(e: &Expr) -> Option<String> {
        if let Expr::Call(call) = e
            && let Some(Expr::Lit(syn::ExprLit {
                lit: Lit::Str(s), ..
            })) = call.args.first()
        {
            Some(s.value())
        } else {
            None
        }
    }

    #[test]
    fn story_impl_generates_a_story_constructor() {
        let out = story_impl(
            quote!(name = "Demo/Widget", active = true, width = 12.0),
            quote! {
                /// A demo widget.
                fn widget(active: bool, width: f64, label: String) -> impl IntoView {
                    view! { <div data-active=active data-width=width>{label}</div> }
                }
            },
        )
        .to_string();

        assert!(
            out.contains("pub fn widget"),
            "emits a constructor named after the fn"
        );
        assert!(
            out.contains("__from_macro"),
            "builds via Story::__from_macro"
        );
        assert!(out.contains("Demo/Widget"), "uses the name override");
        for ty in ["bool", "f64", "String"] {
            assert!(
                out.contains(&format!("< {ty} as theoria :: StoryArg >")),
                "the {ty} arg's control comes from its StoryArg impl; got:\n{out}"
            );
        }
        assert!(out.contains("Some"), "captures the doc description");
    }

    #[test]
    fn story_impl_falls_back_to_the_fn_name_without_an_override() {
        let out =
            story_impl(quote!(), quote! { fn yard() -> impl IntoView { view! {} } }).to_string();
        // No `name = "..."`, so the story name is the fn ident as a string literal.
        assert!(out.contains("\"yard\""), "no override → use the fn name");
    }

    #[test]
    fn story_impl_rejects_self_receivers() {
        let out = story_impl(
            quote!(),
            quote! { fn bad(&self) -> impl IntoView { view! {} } },
        )
        .to_string();
        assert!(
            out.contains("compile_error"),
            "a self receiver → compile error"
        );
    }

    #[test]
    fn doc_description_joins_lines_with_newlines() {
        let f: ItemFn = syn::parse_quote! {
            /// First line.
            /// Second line.
            fn x() -> impl IntoView { view! {} }
        };
        assert_eq!(
            some_str(&doc_description(&f)).as_deref(),
            Some("First line.\nSecond line."),
        );
    }

    #[test]
    fn doc_description_is_none_without_docs() {
        let f: ItemFn = syn::parse_quote! { fn x() -> impl IntoView { view! {} } };
        assert_eq!(some_str(&doc_description(&f)), None);
    }

    #[test]
    fn a_type_label_has_no_spaces_inside_a_generic_type() {
        assert_eq!(type_label(&syn::parse_quote!(Signal<f64>)), "Signal<f64>");
        assert_eq!(
            type_label(&syn::parse_quote!(leptos::prelude::Signal<String>)),
            "leptos::prelude::Signal<String>"
        );
    }

    #[test]
    fn a_type_label_keeps_the_space_between_two_words() {
        assert_eq!(
            type_label(&syn::parse_quote!(Choice<&'static str>)),
            "Choice<&'static str>"
        );
    }

    #[test]
    fn a_type_label_puts_a_space_after_a_comma_or_semicolon() {
        assert_eq!(type_label(&syn::parse_quote!((f64, bool))), "(f64, bool)");
        assert_eq!(type_label(&syn::parse_quote!([f64; 3])), "[f64; 3]");
    }

    #[test]
    fn a_type_label_keeps_each_groups_delimiters() {
        assert_eq!(type_label(&syn::parse_quote!(Grid<{ N }>)), "Grid<{N}>");
        let undelimited = proc_macro2::Group::new(Delimiter::None, quote!(f64));
        assert_eq!(join_tokens(quote!(Signal<#undelimited>)), "Signal<f64>");
    }

    #[test]
    fn a_mut_parameter_is_accepted() {
        let arg = story_arg(&syn::parse_quote!(mut label: String), &HashMap::new());
        assert!(arg.is_ok(), "got: {:?}", arg.err().map(|e| e.to_string()));
    }

    #[test]
    fn a_parameter_that_is_a_pattern_is_rejected() {
        let arg = story_arg(&syn::parse_quote!((a, b): (bool, bool)), &HashMap::new());
        assert!(
            arg.is_err(),
            "a tuple pattern has no one name for its control"
        );
    }
}
