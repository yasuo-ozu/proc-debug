use darling::ast::NestedMeta;
use darling::FromMeta;
use proc_macro::TokenStream as TokenStream1;
use proc_macro2::Span;
use proc_macro2::TokenStream;
use proc_macro_error::{abort, proc_macro_error};
use syn::spanned::Spanned;
use syn::*;
use template_quote::quote;

#[derive(FromMeta, Debug, Clone, PartialEq)]
struct Arguments {
    label: Option<String>,
}

fn check_macro_kind(attrs: &[Attribute]) -> (&'static str, Option<String>) {
    for attr in attrs {
        if let Meta::Path(path) = &attr.meta {
            if path.is_ident("proc_macro") {
                return ("function", None);
            } else if path.is_ident("proc_macro_attribute") {
                return ("attribute", None);
            }
        } else if let Meta::List(MetaList { path, tokens, .. }) = &attr.meta {
            if !path.is_ident("proc_macro_derive") {
                continue;
            }
            if let Ok(ident) = parse2::<Ident>(tokens.clone()) {
                return ("derive", Some(ident.to_string()));
            }
        }
    }
    ("unknown", None)
}

fn inner(args: Arguments, mut input: ItemFn) -> TokenStream {
    let mut macro_inputs = Vec::new();
    let (macro_kind, derive_ident) = check_macro_kind(&input.attrs);
    if let Some(derive_ident) = derive_ident {
        macro_inputs.push(quote!(#derive_ident.to_string()));
    }
    let mut inner_attrs = vec![];
    let mut outer_attrs = vec![];
    for attr in &input.attrs {
        match &attr.meta {
            Meta::Path(path)
            | Meta::List(MetaList { path, .. })
            | Meta::NameValue(MetaNameValue { path, .. })
                if path.is_ident("proc_macro")
                    || path.is_ident("proc_macro_attribute")
                    || path.is_ident("proc_macro_derive")
                    || path.is_ident("proc_macro_error")
                    || path.is_ident("doc") =>
            {
                outer_attrs.push(attr.clone());
            }

            _ => inner_attrs.push(attr.clone()),
        }
    }
    input.attrs = inner_attrs;
    for (n, input) in input.sig.inputs.iter_mut().enumerate() {
        match input {
            FnArg::Typed(pat_type) => {
                let ident = match pat_type.pat.as_ref() {
                    Pat::Ident(pat_ident) if &pat_ident.ident != "_" => pat_ident.ident.clone(),
                    _ => {
                        let ident = Ident::new(&format!("__proc_debug_arg_{}", n), pat_type.span());
                        pat_type.pat = Box::new(Pat::Ident(PatIdent {
                            attrs: vec![],
                            by_ref: None,
                            mutability: None,
                            ident: ident.clone(),
                            subpat: None,
                        }));
                        ident
                    }
                };
                macro_inputs.push(quote!(#ident.to_string()));
            }
            _ => (),
        }
    }
    let (impl_generics, _, where_clause) = input.sig.generics.split_for_impl();
    quote! {
        #(#outer_attrs)*
        #{&input.vis} #{&input.sig.constness} #{&input.sig.asyncness} #{&input.sig.unsafety} #{&input.sig.abi} #{&input.sig.fn_token} #{&input.sig.ident} #impl_generics (
            #{&input.sig.inputs}
            #{&input.sig.variadic}
        ) #{&input.sig.output} #where_clause {
            #input
            ::proc_macro::TokenStream::from(
                ::proc_debug::proc_wrapper(
                    #(if let Some(label) = &args.label) {
                        #label
                    } #(else) {
                        &::std::format!("{}::{}", ::std::module_path!(), #{input.sig.ident.to_string()})
                    },
                    ::std::file!(),
                    ::std::line!() as usize,
                    ::std::module_path!(),
                    #macro_kind,
                    #{input.sig.ident.to_string()},
                    &[ #(for input in &macro_inputs),{#input} ],
                    || {
                        ::proc_macro2::TokenStream::from(
                            #{&input.sig.ident}(
                                #(for arg in &input.sig.inputs) {
                                    #(if let FnArg::Receiver(arg) = arg) {
                                        #{&arg.self_token}
                                    }
                                    #(if let FnArg::Typed(pat_type) = arg) {
                                        #{&pat_type.pat}
                                    },
                                }
                                #{&input.sig.variadic}
                            )
                        )
                    }
                )
            )
        }
    }
}

#[proc_macro_error]
#[proc_macro_attribute]
pub fn proc_debug(attr: TokenStream1, input: TokenStream1) -> TokenStream1 {
    let args = Arguments::from_list(&NestedMeta::parse_meta_list(attr.into()).unwrap()).unwrap();
    inner(
        args,
        parse(input).unwrap_or_else(|_| abort!(Span::call_site(), "Require function")),
    )
    .into()
}
