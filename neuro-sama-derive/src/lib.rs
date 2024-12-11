use proc_macro2::TokenStream;
use quote::{quote, ToTokens};
use syn::{Data, DeriveInput, Fields};

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
                                    desc += &s.value().trim();
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

/// See the `neuro_same` crate for more info.
#[proc_macro_derive(Actions, attributes(name))]
pub fn derive_actions(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    derive_actions2(input.into()).into()
}
