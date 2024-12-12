use proc_macro2::{Group, Span, TokenStream, TokenTree};
use quote::{quote, ToTokens};
use syn::{spanned::Spanned, token::Mut, Data, DeriveInput, Fields, Ident, Item, Path};

fn derive_actions2(input: TokenStream) -> TokenStream {
    let data: DeriveInput = syn::parse2(input).unwrap();
    let name = data.ident;
    let Data::Enum(data) = data.data else {
        panic!("#[derive(Actions)] is only supported on enums")
    };
    let mut ret = TokenStream::new();
    let mut ret1 = TokenStream::new();
    let mut meta = TokenStream::new();
    let mut names = TokenStream::new();
    for variant in data.variants {
        let field = match variant.fields {
            Fields::Unit => None,
            Fields::Unnamed(a) => {
                if a.unnamed.len() > 1 {
                    panic!(
                        "#[derive(Actions)] doesn't support enum variants with more than one field"
                    );
                }
                a.unnamed.into_iter().next()
            }
            Fields::Named(_) => panic!("#[derive(Actions)] doesn't support named fields"),
        };
        if let Some(field) = field {
            let ty = field.ty;
            let ident = variant.ident;
            let mut desc = String::new();
            let mut name = None;
            for attr in variant.attrs {
                match attr.meta.path().to_token_stream().to_string().as_str() {
                    "doc" => {
                        let x = attr.meta.require_name_value().unwrap();
                        match &x.value {
                            syn::Expr::Lit(lit) => match &lit.lit {
                                syn::Lit::Str(s) => {
                                    if !desc.is_empty() {
                                        desc.push('\n');
                                    }
                                    desc += s.value().trim();
                                }
                                _ => panic!("doc comment value is not a string literal???"),
                            },
                            _ => panic!("doc comment value is not a string literal???"),
                        }
                    }
                    "name" => {
                        let x = attr.meta.require_name_value().unwrap();
                        name = Some(x.value.clone());
                    }
                    _ => {}
                }
            }
            if desc.is_empty() {
                panic!("expected variant {} to have a doc comment", ident)
            }
            let name = name
                .ok_or_else(|| {
                    panic!(
                        "expected variant {} to have a #[name = ...] attribute",
                        ident
                    )
                })
                .unwrap();
            ret.extend(quote! {
                impl neuro_sama::game::Action for #ty {
                    fn name() -> &'static str {
                        #name
                    }
                    fn description() -> &'static str {
                        #desc.trim()
                    }
                }
            });
            ret1.extend(quote! {
                #name => <#ty as neuro_sama::serde::Deserialize<'_>>::deserialize(de).map(Self::#ident),
            });
            meta.extend(quote! {
                neuro_sama::schema::Action {
                    name: #name.into(),
                    description: #desc.trim().into(),
                    schema: neuro_sama::schemars::schema_for!(#ty),
                },
            });
            names.extend(quote! { #name.into(), });
        } else {
            panic!("#[derive(Actions)] doesn't support empty variants, since each variant has to be a separate type as well");
        }
    }
    ret.extend(quote! {
        impl<'de> neuro_sama::game::Actions<'de> for #name where Self: 'de  {
            fn deserialize<D: neuro_sama::serde::Deserializer<'de>>(discriminant: &str, de: D) -> Result<Self, D::Error> {
                use neuro_sama::serde::de::Error as _;
                match discriminant {
                    #ret1
                    _ => Err(D::Error::custom(format!("unexpected action: `{discriminant}`"))),
                }
            }
        }
        impl neuro_sama::game::ActionMetadata for #name {
            fn actions() -> Vec<neuro_sama::schema::Action> {
                vec![#meta]
            }
            fn names() -> Vec<std::borrow::Cow<'static, str>> {
                vec![#names]
            }
        }
    });
    ret
}

fn generic_mutability2(attr: TokenStream, input: TokenStream) -> TokenStream {
    let inp: Item = syn::parse2(input).unwrap();
    let mut attr = attr.into_iter();
    let ident = Ident::new(&attr.next().unwrap().to_string(), Span::call_site());
    let (ident, out) = match &inp {
        Item::Struct(inp) => {
            let mut out = inp.clone();
            let ident2 = Ident::new(&attr.nth(1).unwrap().to_string(), Span::call_site());
            match out
                .generics
                .type_params_mut()
                .next()
                .unwrap()
                .bounds
                .first_mut()
                .unwrap()
            {
                syn::TypeParamBound::Trait(tr) => {
                    tr.path.segments.first_mut().unwrap().ident = ident2
                }
                _ => panic!(),
            }
            out.ident = ident;
            (Some(inp.ident.clone()), out.to_token_stream())
        }
        Item::Impl(inp) => {
            let mut out = inp.clone();
            let ident2 = Ident::new(&attr.nth(1).unwrap().to_string(), Span::call_site());
            match out
                .generics
                .type_params_mut()
                .next()
                .unwrap()
                .bounds
                .first_mut()
                .unwrap()
            {
                syn::TypeParamBound::Trait(tr) => {
                    tr.path.segments.first_mut().unwrap().ident = ident2
                }
                _ => panic!(),
            }
            match &mut *out.self_ty {
                syn::Type::Path(x) => {
                    let seg = x.path.segments.first_mut().unwrap();
                    seg.ident = ident;
                }
                _ => panic!(),
            }
            (None, out.to_token_stream())
        }
        Item::Trait(inp) => {
            let mut out = inp.clone();
            out.ident = ident;
            out.attrs.retain(|x| {
                !matches!(
                    x.path().to_token_stream().to_string().as_str(),
                    "generic_mutability" | "doc",
                )
            });
            if attr.next().is_some() {
                if let syn::TypeParamBound::Trait(t) = out.supertraits.first_mut().unwrap() {
                    t.path =
                        Path::from(Ident::new(&attr.next().unwrap().to_string(), t.path.span()))
                }
            }
            for item in &mut out.items {
                if let syn::TraitItem::Fn(x) = item {
                    if let Some(arg) = x.sig.inputs.first_mut() {
                        match arg {
                            syn::FnArg::Receiver(x)
                                if x.mutability.is_none() && x.reference.is_some() =>
                            {
                                x.mutability = Some(Mut {
                                    span: x.reference.as_ref().unwrap().0.span,
                                });
                                if let syn::Type::Reference(x) = &mut *x.ty {
                                    x.mutability = Some(Mut {
                                        span: x.elem.span(),
                                    })
                                }
                            }
                            _ => {}
                        }
                    }
                    // panic!("{}", x.to_token_stream().to_string());
                }
            }
            (Some(inp.ident.clone()), out.to_token_stream())
        }
        _ => panic!(),
    };
    fn hack_stream(x: TokenStream) -> TokenStream {
        x.into_iter().map(hack_tree).collect()
    }
    fn hack_tree(x: TokenTree) -> TokenTree {
        match x {
            TokenTree::Group(x) => {
                let del = x.delimiter();
                let st = x.stream();
                let st = hack_stream(st);
                TokenTree::Group(Group::new(del, st))
            }
            TokenTree::Ident(ref ident) => match ident.to_string().as_str() {
                "ForceActionsBuilder" => {
                    TokenTree::Ident(Ident::new("ForceActionsBuilderMut", Span::call_site()))
                }
                "send_ws_command" => {
                    TokenTree::Ident(Ident::new("send_ws_command_mut", Span::call_site()))
                }
                _ => x,
            },
            TokenTree::Punct(_) => x,
            TokenTree::Literal(_) => x,
        }
    }
    let out = hack_stream(out);

    if let Some(ident) = ident {
        let doc = format!(
            "A mutable version of [`{}`]. See [`{}`] docs for examples.",
            ident, ident
        );
        quote! {
            #[doc = #doc]
            #out
            #inp
        }
    } else {
        quote! {
            #out
            #inp
        }
    }
}

/// See the `neuro_sama` crate for more info.
#[proc_macro_derive(Actions, attributes(name))]
pub fn derive_actions(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_actions2(input.into()).into()
}

#[proc_macro_attribute]
#[doc(hidden)]
pub fn generic_mutability(
    attr: proc_macro::TokenStream,
    input: proc_macro::TokenStream,
) -> proc_macro::TokenStream {
    generic_mutability2(attr.into(), input.into()).into()
}
