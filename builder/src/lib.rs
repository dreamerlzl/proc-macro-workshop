use proc_macro2::TokenStream;
use quote::quote;
use syn::{parse_macro_input, Data, DeriveInput, Fields, Ident};

#[proc_macro_derive(Builder)]
pub fn derive(input: proc_macro::TokenStream) -> proc_macro::TokenStream {
    let tree = parse_macro_input!(input as DeriveInput);
    let name = tree.ident;
    let builder_name = Ident::new(&format!("{}Builder", name), name.span());
    let build_struct_def = build_struct_def(&name, &builder_name, &tree.data);
    let tokens = quote! {
        impl #name {
            pub fn builder () -> #builder_name {
                #builder_name::default()
            }
        }

        #build_struct_def
    };
    tokens.into()
}

fn build_struct_def(name: &Ident, builder_name: &Ident, data: &Data) -> TokenStream {
    let  Data::Struct(data) = data else {
        unimplemented!()
    };
    let Fields::Named(ref fields) = data.fields else {
        unimplemented!()
    };
    let mut field_list = Vec::new();
    let mut method_list = Vec::new();
    let mut field_check_list = Vec::new();
    let mut assign_field = Vec::new();
    for field in fields.named.iter() {
        if let Some(ref name) = field.ident {
            let ty = &field.ty;
            field_list.push(quote! {
                #name: Option<#ty>,
            });
            method_list.push(quote! {
                fn #name (&mut self, value: #ty) -> &mut Self {
                    self.#name = Some(value);
                    self
                }
            });
            field_check_list.push(quote! {
                self.#name.is_none()
            });
            assign_field.push(quote! {
                #name: self.#name.take().unwrap(),
            });
        }
    }
    quote! {
        #[derive(Default, Debug)]
        pub struct #builder_name {
            #(#field_list)*
        }

        impl #builder_name {
            #(#method_list)*

            pub fn build(&mut self) -> Result<#name, Box<dyn std::error::Error>> {
                if (false #( || #field_check_list)*) {
                    return Err("all fields must be non-empty".into());
                }
                Ok(#name {
                    #(#assign_field)*
                })
            }
        }
    }
}
