use proc_macro::TokenStream;
use proc_macro2::Span;
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Data, DataStruct, Error, Fields, Ident, LitInt, LitStr, Result, Token,
};

pub fn derive_vertex(ast: syn::DeriveInput) -> Result<TokenStream> {
    let struct_name = &ast.ident;

    let fields = match &ast.data {
        Data::Struct(DataStruct {
            fields: Fields::Named(fields),
            ..
        }) => &fields.named,
        _ => {
            return Err(Error::new_spanned(
                ast,
                "expected a struct with named fields",
            ));
        }
    };

    let crate_ident = match crate_name("screen-13-macros") {
        Ok(FoundCrate::Itself) => Ident::new("screen_13_macros", Span::call_site()),
        Ok(FoundCrate::Name(name)) => Ident::new(&name, Span::call_site()),
        _ => {
            return Err(Error::new_spanned(
                ast,
                "expected screen-13 to be present in `Cargo.toml`",
            ))
        }
    };

    let mut attributes = quote! {
        let mut offset = 0;
        let mut attributes = HashMap::default();
    };

    for field in fields.iter() {
        let field_name = field.ident.to_owned().unwrap();
        let field_ty = &field.ty;
        // By default we use the field-name to match the shader input
        let mut names = vec![LitStr::new(&field_name.to_string(), Span::call_site())];
        // We predefine the format as empty, if it remains empty we throw an error
        let mut format = quote! {};

        for attr in &field.attrs {
            let attr_ident = if let Some(ident) = attr.path.get_ident() {
                ident
            } else {
                continue;
            };

            if attr_ident == "name" {
                let meta = attr.parse_args_with(NameMeta::parse)?;
                names = meta.names_lit_str.into_iter().collect();
            } else if attr_ident == "format" {
                let meta = attr.parse_args_with(FormatMeta::parse)?;
                match meta {
                    FormatMeta::FormatOnly(format_ident) => {
                        format = quote! {
                            let format = vk::Format::#format_ident;
                            let num_locations = 1;
                        };
                    }
                    FormatMeta::FormatWithLocationCount(FormatWithLocationCount {
                        format_ident,
                        _comma,
                        count_lit_int,
                    }) => {
                        format = quote! {
                            let format = vk::Format::#format_ident;
                            let num_locations = #count_lit_int as u32;
                        };
                    }
                }
            }
        }

        if format.is_empty() {
            // no format is specified, so we treat it as padding
            names = Vec::new();
        }

        for name in &names {
            attributes = quote! {
                #attributes
                {
                    #format
                    attributes.insert(
                        #name.to_string(),
                        DerivedVertexAttribute {
                            block_size: std::mem::size_of::<#field_ty>() as u32 / num_locations,
                            offset,
                            format,
                            num_locations,
                        },
                    );
                }
            };
        }

        // Before we process the next field we increment our offset
        attributes = quote! {
            #attributes
            offset += std::mem::size_of::<#field_ty>() as u32;
        }
    }

    Ok(TokenStream::from(quote! {
        #[allow(unsafe_code)]
        impl #crate_ident::vertex::Vertex for #struct_name {
            #[inline]
            fn layout(input_rate: #crate_ident::ash::vk::VertexInputRate) -> #crate_ident::vertex::DerivedVertexLayout {
                use std::collections::HashMap;
                use #crate_ident::ash::vk;
                use #crate_ident::vertex::DerivedVertexAttribute;

                #attributes

                #crate_ident::vertex::DerivedVertexLayout {
                    attributes,
                    input_rate,
                    stride: std::mem::size_of::<#struct_name>() as u32,
                }
            }
        }
    }))
}

struct NameMeta {
    names_lit_str: Punctuated<LitStr, Token![,]>,
}

impl Parse for NameMeta {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            names_lit_str: input.parse_terminated(<LitStr as Parse>::parse)?,
        })
    }
}

enum FormatMeta {
    FormatOnly(Ident),
    FormatWithLocationCount(FormatWithLocationCount),
}

impl Parse for FormatMeta {
    fn parse(input: ParseStream) -> Result<Self> {
        if input.peek2(Token![,]) {
            input.parse().map(Self::FormatWithLocationCount)
        } else {
            input.parse().map(Self::FormatOnly)
        }
    }
}

struct FormatWithLocationCount {
    format_ident: Ident,
    _comma: Token![,],
    count_lit_int: LitInt,
}

impl Parse for FormatWithLocationCount {
    fn parse(input: ParseStream) -> Result<Self> {
        Ok(Self {
            format_ident: input.parse()?,
            _comma: input.parse()?,
            count_lit_int: input.parse()?,
        })
    }
}
