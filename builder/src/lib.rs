use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parse_macro_input, spanned::Spanned, Attribute, Data, DeriveInput, Error, Fields, Ident,
    MetaList, MetaNameValue, NestedMeta, PathSegment, Result, Type,
};

#[proc_macro_derive(Builder, attributes(builder))]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let tree = parse_macro_input!(input as DeriveInput);
    match build_struct_def(tree) {
        Ok(tks) => tks.into(),
        Err(e) => proc_macro::TokenStream::from(e.to_compile_error()),
    }
}

fn build_struct_def(tree: DeriveInput) -> Result<TokenStream> {
    let span = tree.span();
    let  Data::Struct(data) = tree.data else {
        return Err(Error::new(span, "Builder derive macro only supports struct"));
    };
    let Fields::Named(ref fields) = data.fields else {
        return Err(Error::new(span, "Builder derive macro only supports named fields"));
    };
    let vec_len = fields.named.len();
    let mut field_list = Vec::with_capacity(vec_len);
    let mut builder_init_list = Vec::with_capacity(vec_len);
    let mut method_list = Vec::with_capacity(vec_len);
    let mut assign_field = Vec::with_capacity(vec_len);

    for field in fields.named.iter() {
        if let Some(ref name) = field.ident {
            let ty = &field.ty;
            match check_field_type(ty) {
                FieldType::OptionType(raw_ty) => {
                    builder_init_list.push(quote! {
                        #name: None,
                    });
                    field_list.push(quote! {
                        #name: ::core::option::Option<#raw_ty>,
                    });
                    assign_field.push(quote! {
                        #name: self.#name.clone(),
                    });
                    method_list.push(quote! {
                        fn #name (&mut self, value: #raw_ty) -> &mut Self {
                            self.#name = ::core::option::Option::Some(value);
                            self
                        }
                    });
                }
                FieldType::VecType(raw_ty) => {
                    // check whether there is an argument "builder"
                    if let Some(each) = get_builder_each(&field.attrs)? {
                        builder_init_list.push(quote! {
                            #name:  std::vec::Vec::new(),
                        });
                        field_list.push(quote! {
                            #name: #ty,
                        });
                        method_list.push(quote! {
                            fn #each (&mut self, value: #raw_ty) -> &mut Self {
                                self.#name.push(value);
                                self
                            }
                        });
                        assign_field.push(quote! {
                            #name: self.#name.drain(..).collect(),
                        });
                    } else {
                        builder_init_list.push(quote! {
                            #name: None,
                        });
                        field_list.push(quote! {
                            #name: ::core::option::Option<#ty>,
                        });
                        assign_field.push(quote! {
                            #name: self.#name.take().ok_or(concat!(stringify!(#name), "is not set"))?,
                        });
                        method_list.push(quote! {
                            fn #name (&mut self, value: #ty) -> &mut Self {
                                self.#name = ::core::option::Option::Some(value);
                                self
                            }
                        });
                    }
                }
                FieldType::RawType(_) => {
                    builder_init_list.push(quote! {
                        #name: None,
                    });
                    field_list.push(quote! {
                        #name: ::core::option::Option<#ty>,
                    });
                    assign_field.push(quote! {
                        #name: self.#name.take().ok_or(concat!(stringify!(#name), "is not set"))?,
                    });
                    method_list.push(quote! {
                        fn #name (&mut self, value: #ty) -> &mut Self {
                            self.#name = ::core::option::Option::Some(value);
                            self
                        }
                    });
                }
                FieldType::UnsupportedType => {
                    return Err(Error::new(field.span(), "unsupported field type"));
                }
            }
        }
    }

    let name = tree.ident;
    let builder_name = Ident::new(&format!("{}Builder", name), name.span());
    let result = quote! {
        impl #name {
            pub fn builder() -> #builder_name {
                #builder_name {
                    #(#builder_init_list)*
                }
            }
        }

        pub struct #builder_name {
            #(#field_list)*
        }

        impl #builder_name {
            #(#method_list)*

            pub fn build(&mut self) -> std::result::Result<#name, std::boxed::Box<dyn std::error::Error>> {
                std::result::Result::Ok(#name {
                    #(#assign_field)*
                })
            }
        }
    };
    Ok(result)
}

#[allow(clippy::enum_variant_names)]
enum FieldType {
    OptionType(Type),
    VecType(Type),
    RawType(Type),
    UnsupportedType,
}

fn check_field_type(ty: &Type) -> FieldType {
    use syn::{AngleBracketedGenericArguments, GenericArgument, Path, PathArguments, TypePath};
    use FieldType::*;

    if let syn::Type::Path(TypePath {
        path: Path { segments, .. },
        ..
    }) = ty
    {
        if let Some(&PathSegment {
            ref ident,
            arguments:
                PathArguments::AngleBracketed(AngleBracketedGenericArguments { ref args, .. }),
        }) = segments.first()
        {
            if let (1, Some(GenericArgument::Type(t))) = (args.len(), args.first()) {
                return match ident.to_string().as_str() {
                    "Vec" => VecType(t.clone()),
                    "Option" => OptionType(t.clone()),
                    _ => UnsupportedType,
                };
            }
        }
    }
    RawType(ty.clone())
}

// check whether a builder(each = "name") attribute is annotated
// if user provides something like #[builder(eac = "arg")], then we should report an error instead
// of ignoring it
fn get_builder_each(attrs: &[Attribute]) -> Result<Option<Ident>> {
    for attr in attrs.iter() {
        let Ok(meta) = attr.parse_meta() else {
            return Ok(None);
        };
        let meta_span = meta.span();
        match meta {
            syn::Meta::List(MetaList { path, nested, .. }) if path.is_ident("builder") => {
                if let Some(NestedMeta::Meta(syn::Meta::NameValue(MetaNameValue {
                    lit,
                    path,
                    ..
                }))) = nested.first()
                {
                    match lit {
                        syn::Lit::Str(s) if path.is_ident("each") => {
                            return Ok(Some(format_ident!("{}", s.value())));
                        }
                        // if the stuff inside builder is not each, report an error
                        _ => {
                            return Err(Error::new(
                                meta_span,
                                r#"expected `builder(each = "...")`"#,
                            ))
                        }
                    };
                }
            }
            _ => continue,
        }
    }
    Ok(None)
}
