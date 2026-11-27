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
//! macro emits the same code for every type. `#[derive(StoryArg)]` makes a
//! struct one such type, with a control per field.

use proc_macro::TokenStream;
use proc_macro2::{Delimiter, Span, TokenStream as TokenStream2, TokenTree};
use quote::{ToTokens, format_ident, quote, quote_spanned};
use std::collections::{HashMap, HashSet};
use syn::{
    Attribute, Data, DeriveInput, Expr, FnArg, Ident, ItemFn, Lit, Meta, Pat, Token, Type,
    ext::IdentExt, parse::Parser, punctuated::Punctuated, spanned::Spanned,
};

#[proc_macro_attribute]
pub fn story(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Thin adapter to the testable `proc_macro2` core; all logic lives in
    // `story_impl`.
    story_impl(attr.into(), item.into()).into()
}

/// A struct that derives `StoryArg` is a `#[story]` arg with a control per
/// field; theoria's `StoryArg` trait documents it.
#[proc_macro_derive(StoryArg, attributes(story_arg))]
pub fn derive_story_arg(item: TokenStream) -> TokenStream {
    syn::parse2(item.into())
        .and_then(|input| story_arg_impl(&input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
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

    let params: HashSet<String> = func.sig.inputs.iter().filter_map(param_name).collect();

    // `name = "..."` sets the story name; any other `key = expr` is a default for
    // the matching arg.
    let mut name_override: Option<Expr> = None;
    let mut defaults: HashMap<String, Expr> = HashMap::new();
    for meta in overrides {
        let Meta::NameValue(nv) = meta else {
            return syn::Error::new_spanned(meta, "a #[story] entry is written `key = value`")
                .to_compile_error();
        };
        let key = nv
            .path
            .get_ident()
            .map_or_else(|| nv.path.to_token_stream().to_string(), arg_name);
        if key == "name" {
            name_override = Some(nv.value);
        } else if params.contains(&key) {
            defaults.insert(key, nv.value);
        } else {
            return syn::Error::new_spanned(
                &nv.path,
                format!("`{key}` is not a parameter of this story"),
            )
            .to_compile_error();
        }
    }

    let fname = func.sig.ident.clone();
    let story_name: Expr = name_override.unwrap_or_else(|| {
        let s = fname.to_string();
        syn::parse_quote!(#s)
    });
    let description = doc_description(&func.attrs);

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

fn param_name(input: &FnArg) -> Option<String> {
    let FnArg::Typed(pt) = input else {
        return None;
    };
    let Pat::Ident(pi) = &*pt.pat else {
        return None;
    };
    Some(arg_name(&pi.ident))
}

fn arg_name(ident: &Ident) -> String {
    ident.unraw().to_string()
}

/// What one story parameter adds to the generated constructor.
struct ArgTokens {
    /// Outside the view closure: the arg and the signal it feeds.
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
    let name = arg_name(ident);
    if name == "name" {
        return Err(syn::Error::new_spanned(
            ident,
            "a #[story] parameter cannot be called `name`, which the attribute uses for the story's name",
        ));
    }
    let arg_ident = format_ident!("__arg_{}", ident);
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
            let (#read_ident, #arg_ident) =
                <#arg::Value as theoria::__private::Plain>::arg(#name, #label, #default);
        },
        get_let: quote_spanned!(span=> let #mutability #ident: #ty = #arg::bind(#read_ident);),
        entry: quote!(#arg_ident),
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
fn doc_description(attrs: &[Attribute]) -> Expr {
    match doc_text(attrs) {
        Some(doc) => syn::parse_quote!(::std::option::Option::Some(#doc)),
        None => syn::parse_quote!(::std::option::Option::None),
    }
}

/// The doc-comment lines, trimmed and joined with newlines.
fn doc_text(attrs: &[Attribute]) -> Option<String> {
    let mut doc = String::new();
    for a in attrs {
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
    (!doc.trim().is_empty()).then_some(doc)
}

/// How a field of a struct that derives `StoryArg` gets its value.
#[derive(Debug, PartialEq)]
enum FieldKind {
    /// Through `theoria::__private::Field`, by its type.
    Typed,
    /// A `&'static str` drawn as a color.
    Color,
    /// No control: the start value, kept.
    Skip,
}

/// The `#[story_arg(...)]` options on one item, each with where it was
/// written.
#[derive(Default)]
struct Options {
    color: Option<Span>,
    skip: Option<Span>,
}

fn options(attrs: &[Attribute], on_a_field: bool) -> syn::Result<Options> {
    let mut found = Options::default();
    for attr in attrs.iter().filter(|a| a.path().is_ident("story_arg")) {
        attr.parse_nested_meta(|meta| {
            let span = meta.path.span();
            let key = meta
                .path
                .get_ident()
                .map(ToString::to_string)
                .unwrap_or_default();
            let slot = match key.as_str() {
                "color" => &mut found.color,
                "skip" if on_a_field => &mut found.skip,
                _ => {
                    let expected = if on_a_field {
                        "`color` or `skip`"
                    } else {
                        "`color`"
                    };
                    return Err(meta.error(format!(
                        "`{}` is not a story_arg option here; expected {expected}",
                        meta.path.to_token_stream(),
                    )));
                }
            };
            if slot.replace(span).is_some() {
                return Err(meta.error(format!("`{key}` is given twice")));
            }
            Ok(())
        })?;
    }
    Ok(found)
}

/// Whether `ty` is written `&'static str`.
fn is_static_str(ty: &Type) -> bool {
    let reference = match ty {
        Type::Reference(reference) => reference,
        Type::Group(group) => return is_static_str(&group.elem),
        Type::Paren(paren) => return is_static_str(&paren.elem),
        _ => return false,
    };
    reference.mutability.is_none()
        && reference
            .lifetime
            .as_ref()
            .is_some_and(|lifetime| lifetime.ident == "static")
        && matches!(&*reference.elem, Type::Path(path) if path.qself.is_none() && path.path.is_ident("str"))
}

/// The struct's named fields.
fn named_fields(input: &DeriveInput) -> syn::Result<&Punctuated<syn::Field, Token![,]>> {
    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            &input.generics,
            "derive(StoryArg) takes a struct with no generic parameters",
        ));
    }
    if let Some(clause) = &input.generics.where_clause {
        return Err(syn::Error::new_spanned(
            clause,
            "derive(StoryArg) takes a struct with no where clause",
        ));
    }
    match &input.data {
        Data::Struct(syn::DataStruct {
            fields: syn::Fields::Named(fields),
            ..
        }) => Ok(&fields.named),
        _ => Err(syn::Error::new_spanned(
            &input.ident,
            "derive(StoryArg) takes a struct with named fields",
        )),
    }
}

/// How each field gets its value, from the struct's and the fields'
/// `#[story_arg(...)]` options.
fn field_kinds(
    input: &DeriveInput,
    fields: &Punctuated<syn::Field, Token![,]>,
) -> syn::Result<Vec<FieldKind>> {
    let ident = &input.ident;
    let every_str_a_color = options(&input.attrs, false)?.color;
    let mut kinds = Vec::new();
    let mut colors_a_str = false;
    for field in fields {
        let found = options(&field.attrs, true)?;
        let name = field.ident.as_ref().map_or_else(String::new, arg_name);
        kinds.push(match (found.color, found.skip) {
            (Some(_), Some(skip)) => {
                return Err(syn::Error::new(
                    skip,
                    format!("`{name}` is skipped, so it cannot also be a color"),
                ));
            }
            (Some(color), None) if every_str_a_color.is_some() && is_static_str(&field.ty) => {
                return Err(syn::Error::new(
                    color,
                    format!("`{name}` is already a color, from the struct's #[story_arg(color)]"),
                ));
            }
            (Some(_), None) => FieldKind::Color,
            (None, Some(_)) => FieldKind::Skip,
            (None, None) if every_str_a_color.is_some() && is_static_str(&field.ty) => {
                colors_a_str = true;
                FieldKind::Color
            }
            (None, None) => FieldKind::Typed,
        });
    }
    if let Some(color) = every_str_a_color
        && !colors_a_str
    {
        return Err(syn::Error::new(
            color,
            format!("`{ident}` has no `&'static str` field to make a color"),
        ));
    }
    if kinds.iter().all(|kind| matches!(kind, FieldKind::Skip)) {
        return Err(syn::Error::new_spanned(
            ident,
            format!("`{ident}` has no field with a control"),
        ));
    }
    Ok(kinds)
}

fn story_arg_impl(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let ident = &input.ident;
    let fields = named_fields(input)?;
    let kinds = field_kinds(input, fields)?;

    let mut lets = Vec::new();
    let mut args = Vec::new();
    let mut values = Vec::new();
    for (index, (field, kind)) in fields.iter().zip(&kinds).enumerate() {
        let member = field.ident.as_ref().expect("a named field");
        // Named by position, not by the field, so a field the crate allows to
        // be camelCase does not give the derive a local that warns.
        let start = format_ident!("__start_{}", index);
        lets.push(quote!(let #start = ::core::clone::Clone::clone(&start.#member);));
        let (name, label, ty) = (arg_name(member), type_label(&field.ty), &field.ty);
        let build = match kind {
            FieldKind::Skip => {
                values.push(quote!(#member: ::core::clone::Clone::clone(&#start)));
                continue;
            }
            FieldKind::Typed => quote_spanned!(ty.span()=>
                <#ty as theoria::__private::Field>::field(#name, #label, #start)
            ),
            FieldKind::Color => quote_spanned!(ty.span()=>
                <#ty as theoria::__private::ColorField>::color_field(#name, #label, #start)
            ),
        };
        let (read, arg) = (
            format_ident!("__read_{}", index),
            format_ident!("__arg_{}", index),
        );
        let described =
            doc_text(&field.attrs).map(|doc| quote!(let #arg = #arg.with_description(#doc);));
        lets.push(quote! {
            let (#read, #arg) = #build;
            #described
        });
        args.push(arg);
        values.push(quote!(#member: theoria::__private::read(#read)));
    }

    Ok(quote! {
        #[automatically_derived]
        impl theoria::__private::Sealed for #ident {}

        #[automatically_derived]
        impl theoria::StoryArg for #ident {
            type Value = Self;
            type Init = Self;

            fn bind(signal: theoria::__private::Signal<Self>) -> Self {
                theoria::__private::read(signal)
            }
        }

        #[automatically_derived]
        impl theoria::__private::Plain for #ident {
            fn arg(
                name: &'static str,
                ty: &'static str,
                start: Self,
            ) -> (theoria::__private::Signal<Self>, theoria::Arg) {
                #(#lets)*
                theoria::__private::group(name, ty, ::std::vec![#(#args),*], move || Self {
                    #(#values),*
                })
            }
        }
    })
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
            some_str(&doc_description(&f.attrs)).as_deref(),
            Some("First line.\nSecond line."),
        );
    }

    #[test]
    fn doc_description_is_none_without_docs() {
        let f: ItemFn = syn::parse_quote! { fn x() -> impl IntoView { view! {} } };
        assert_eq!(some_str(&doc_description(&f.attrs)), None);
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

    fn expand(attr: TokenStream2, params: &TokenStream2) -> String {
        story_impl(attr, quote! { fn x(#params) -> impl IntoView { view! {} } }).to_string()
    }

    #[test]
    fn a_default_for_a_parameter_the_story_has_is_accepted() {
        let out = expand(quote!(width = 12.0), &quote!(width: f64));
        assert!(!out.contains("compile_error"), "got: {out}");
    }

    #[test]
    fn a_default_for_a_parameter_the_story_lacks_is_rejected() {
        let out = expand(quote!(widht = 12.0), &quote!(width: f64));
        assert!(out.contains("compile_error"), "got: {out}");
    }

    #[test]
    fn the_error_for_a_stray_default_names_its_key() {
        let out = expand(quote!(widht = 12.0), &quote!(width: f64));
        assert!(out.contains("`widht`"), "got: {out}");
    }

    #[test]
    fn an_attribute_entry_with_no_value_is_rejected() {
        let out = expand(quote!(active), &quote!(active: bool));
        assert!(out.contains("compile_error"), "got: {out}");
    }

    #[test]
    fn a_parameter_called_name_is_rejected() {
        let out = expand(quote!(), &quote!(name: String));
        assert!(out.contains("compile_error"), "got: {out}");
    }

    #[test]
    fn a_name_in_the_attribute_is_not_taken_for_a_stray_default() {
        let out = expand(quote!(name = "Demo/Width"), &quote!(width: f64));
        assert!(!out.contains("compile_error"), "got: {out}");
    }

    #[test]
    fn a_raw_identifier_parameter_is_named_without_its_prefix() {
        assert_eq!(
            param_name(&syn::parse_quote!(r#type: String)).as_deref(),
            Some("type")
        );
    }

    #[test]
    fn a_default_keyed_by_a_raw_identifier_is_accepted() {
        let out = expand(quote!(r#type = "button"), &quote!(r#type: String));
        assert!(!out.contains("compile_error"), "got: {out}");
    }

    #[test]
    fn a_mut_parameter_is_accepted() {
        let arg = story_arg(&syn::parse_quote!(mut label: String), &HashMap::new());
        assert!(arg.is_ok(), "got: {:?}", arg.err().map(|e| e.to_string()));
    }

    /// The derive's error for `item`, which must be refused.
    fn refused(item: TokenStream2) -> String {
        match story_arg_impl(&syn::parse2(item).expect("a parsable item")) {
            Ok(out) => panic!("accepted; got:\n{out}"),
            Err(e) => e.to_string(),
        }
    }

    fn kinds(item: TokenStream2) -> Vec<FieldKind> {
        let input: DeriveInput = syn::parse2(item).expect("a parsable item");
        let fields = named_fields(&input).unwrap_or_else(|e| panic!("refused: {e}"));
        field_kinds(&input, fields).unwrap_or_else(|e| panic!("refused: {e}"))
    }

    #[test]
    fn the_derive_refuses_an_enum() {
        let err = refused(quote!(
            enum Theme {
                Day,
            }
        ));
        assert!(err.contains("named fields"), "got: {err}");
    }

    #[test]
    fn the_derive_refuses_a_tuple_struct() {
        let err = refused(quote!(
            struct Theme(&'static str);
        ));
        assert!(err.contains("named fields"), "got: {err}");
    }

    #[test]
    fn the_derive_refuses_a_generic_struct() {
        let err = refused(quote!(
            struct Theme<T> {
                ink: T,
            }
        ));
        assert!(err.contains("no generic parameters"), "got: {err}");
    }

    #[test]
    fn the_derive_refuses_a_where_clause() {
        let err = refused(quote!(
            struct Theme
            where
                u8: Copy,
            {
                ink: &'static str,
            }
        ));
        assert!(err.contains("no where clause"), "got: {err}");
    }

    #[test]
    fn an_unknown_field_option_is_refused_by_name() {
        let err = refused(quote!(
            struct Theme {
                #[story_arg(colour)]
                ink: &'static str,
            }
        ));
        assert!(err.contains("`colour`"), "got: {err}");
    }

    #[test]
    fn skip_on_the_struct_is_refused() {
        let err = refused(quote!(
            #[story_arg(skip)]
            struct Theme {
                ink: &'static str,
            }
        ));
        assert!(err.contains("`skip`"), "got: {err}");
    }

    #[test]
    fn an_option_given_twice_is_refused() {
        let err = refused(quote!(
            struct Theme {
                #[story_arg(skip, skip)]
                ink: &'static str,
                font: &'static str,
            }
        ));
        assert!(err.contains("twice"), "got: {err}");
    }

    #[test]
    fn a_skipped_color_is_refused_by_the_fields_name() {
        let err = refused(quote!(
            struct Theme {
                #[story_arg(color, skip)]
                ink: &'static str,
                font: &'static str,
            }
        ));
        assert!(err.contains("`ink` is skipped"), "got: {err}");
    }

    #[test]
    fn a_color_field_in_a_color_struct_is_refused_by_the_fields_name() {
        let err = refused(quote!(
            #[story_arg(color)]
            struct Theme {
                #[story_arg(color)]
                ink: &'static str,
            }
        ));
        assert!(err.contains("`ink` is already a color"), "got: {err}");
    }

    #[test]
    fn a_color_struct_with_no_str_field_is_refused() {
        let err = refused(quote!(
            #[story_arg(color)]
            struct Theme {
                ink: String,
            }
        ));
        assert!(err.contains("no `&'static str` field"), "got: {err}");
    }

    #[test]
    fn a_color_struct_whose_str_fields_are_all_skipped_is_refused() {
        let err = refused(quote!(
            #[story_arg(color)]
            struct Theme {
                #[story_arg(skip)]
                ink: &'static str,
                size: f64,
            }
        ));
        assert!(err.contains("no `&'static str` field"), "got: {err}");
    }

    #[test]
    fn color_on_a_string_field_in_a_color_struct_is_left_to_the_type_check() {
        let kinds = kinds(quote!(
            #[story_arg(color)]
            struct Theme {
                ink: &'static str,
                #[story_arg(color)]
                label: String,
            }
        ));
        assert_eq!(kinds, [FieldKind::Color, FieldKind::Color]);
    }

    #[test]
    fn a_color_struct_draws_only_its_str_fields_as_colors() {
        let kinds = kinds(quote!(
            #[story_arg(color)]
            struct Theme {
                ink: &'static str,
                label: String,
                #[story_arg(skip)]
                version: u32,
            }
        ));
        assert_eq!(kinds, [FieldKind::Color, FieldKind::Typed, FieldKind::Skip]);
    }

    #[test]
    fn without_options_every_field_is_drawn_by_its_type() {
        let kinds = kinds(quote!(
            struct Theme {
                ink: &'static str,
                label: String,
            }
        ));
        assert_eq!(kinds, [FieldKind::Typed, FieldKind::Typed]);
    }

    #[test]
    fn a_struct_with_every_field_skipped_is_refused() {
        let err = refused(quote!(
            struct Theme {
                #[story_arg(skip)]
                version: u32,
            }
        ));
        assert!(err.contains("no field with a control"), "got: {err}");
    }

    #[test]
    fn a_static_str_is_told_from_other_references() {
        let cases: [(Type, bool); 6] = [
            (syn::parse_quote!(&'static str), true),
            (syn::parse_quote!(&'static mut str), false),
            (syn::parse_quote!(&'a str), false),
            (syn::parse_quote!(&'static String), false),
            (syn::parse_quote!(String), false),
            (syn::parse_quote!((&'static str)), true),
        ];
        for (ty, expected) in cases {
            assert_eq!(is_static_str(&ty), expected, "{}", type_label(&ty));
        }
    }

    // How a `$t:ty` from `macro_rules!` arrives.
    #[test]
    fn a_static_str_in_an_undelimited_group_is_a_static_str() {
        let ty = Type::Group(syn::TypeGroup {
            attrs: Vec::new(),
            group_token: syn::token::Group::default(),
            elem: Box::new(syn::parse_quote!(&'static str)),
        });
        assert!(is_static_str(&ty));
    }

    #[test]
    fn a_blank_doc_comment_is_no_description() {
        assert_eq!(doc_text(&[syn::parse_quote!(#[doc = "  "])]), None);
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
