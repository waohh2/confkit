use proc_macro::TokenStream;
use quote::{quote, ToTokens};
use syn::{punctuated::Punctuated, Meta, Token, parse_macro_input, DeriveInput, Fields, LitStr};

// As I am new to creating procedural macros I made heavy use of this great github blog post by Peter L:
// https://cetra3.github.io/blog/creating-your-own-derive-macro/
// Some of the code in this file is taken from their examples.

struct StructAttrs {
    dir_name: Option<String>,
    file_name: Option<String>,
}

struct FieldInfo {
    name: syn::Ident,
    ty: syn::Type,
    attrs: FieldAttrs,
} 
    
struct FieldAttrs {
    comment: Option<String>,
    default: Option<syn::Expr>,
    nested: bool,
}

#[proc_macro_derive(derive, attributes(detail, config))]
pub fn main(input :TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    
    // This ensures only structs will be compatible, otherwise it will trigger a fallback which outputs a compile error.
    if let syn::Data::Struct(ref data) = input.data {
        // This further filters to only process named fields
        if let Fields::Named(ref fields) = data.fields {
            
            let mut errors :Option<syn::Error> = None;
            
            let struct_attrs = match parse_struct_attrs(&input.attrs) {
                Ok(attrs) => attrs,
                Err(error) => {
                    errors = Some(error);
                    StructAttrs {
                        dir_name: None,
                        file_name: None,
                    }
                },
            };
            
            let field_metadata = match parse_field_attrs(fields) {
                Ok(data) => data,
                Err(error) => {
                    match errors {
                        Some(_) => add_error(&mut errors, error),
                        None => errors = Some(error),
                    }
                    Vec::new()
                },
            };
            
            if errors.is_some() {
                return TokenStream::from(errors.unwrap().to_compile_error())
            }
                
            let name = input.ident;
            
            let dir_name = struct_attrs.dir_name.unwrap_or_else(|| {name.to_string().to_lowercase()} );
            let file_name = struct_attrs.file_name.unwrap_or_else(|| {"config.toml".to_string()} );
            
            let field_names = field_metadata.iter().map(|i| {
                let name = &i.name;
                quote!(#name) 
            });
            
            let serialize_fields = field_metadata.iter().map(|i| {
                let name = &i.name;
                let name_as_string = name.to_string();
                
                quote!(
                    state.serialize_field(
                        #name_as_string,
                        &self.#name,
                    )?;
                )    
            });
            
            let field_count = field_metadata.len();
                      
            // This data is used to construct a seperate but identical struct later. 
            // this allows the code to implement serde derives without requiring the user to manually add serde as a dependency or add their derives.
            let helper_name = syn::Ident::new(&format!("{}Serde", name), name.span());
            let helper_fields = field_metadata.iter().map(|i| {
                let name = &i.name;
                let ty = &i.ty;
                
                quote!(
                    #name: #ty
                )
            });
            
            let default_fields_1 = field_metadata.iter().map(|i| {
                let name = &i.name;
                
                match &i.attrs.default {
                    Some(default) => quote!(
                        #name: #default
                    ),
                    None => quote!(
                        #name: ::std::default::Default::default()
                    ),
                }    
            });
            
            let default_fields_2 = default_fields_1.clone();
            
            let comments = field_metadata.iter().map(|i| {
                let key = i.name.to_string();
                
                let comment = match &i.attrs.comment {
                    Some(comment) => quote!(
                        Some(#comment)
                    ),
                    None => quote!(
                        None
                    ),
                };
                
                let children = if i.attrs.nested {
                    let ty = &i.ty;
                    quote!(
                        <#ty as ::confkit::config_file::Config>::comments()
                    )
                } else {
                    quote!(
                        Vec::new()
                    )
                };
                
                quote!(
                    ::confkit::private::Comment {
                        key: #key,
                        text: #comment,
                        children: #children,
                    }
                )
            });
                                  
            return TokenStream::from(quote!(   
                #[derive(::confkit::private::serde::Deserialize, ::confkit::private::serde::Serialize)]
                #[serde(crate = "::confkit::private::serde")]
                struct #helper_name {
                    #(#helper_fields,)*
                }
                impl Default for #helper_name {
                    fn default() -> Self {
                        Self {
                            #(#default_fields_1,)*
                        }
                    }
                }
                impl Default for #name {
                    fn default() -> Self {
                        Self {
                            #(#default_fields_2,)*
                        }
                    }
                }
                impl ::confkit::private::serde::Serialize for #name {
                    fn serialize<S>(
                        &self,
                        serializer: S,
                    ) -> Result<S::Ok, S::Error>
                    where S: ::confkit::private::serde::Serializer,
                    {
                        use ::confkit::private::serde::ser::SerializeStruct;
                        
                        let mut state = serializer.serialize_struct(
                            stringify!(#name),
                            #field_count,
                        )?;
                        
                        #(#serialize_fields)*
                        state.end()
                    }
                }
                impl<'de> ::confkit::private::serde::Deserialize<'de> for #name {
                    fn deserialize<D>(
                        deserializer: D,
                    ) -> Result<Self, D::Error>
                    where D: ::confkit::private::serde::Deserializer<'de>, {
                        let data = #helper_name::deserialize(deserializer)?;
                        
                        Ok(Self {
                            #(
                                #field_names: data.#field_names,    
                            )*
                        })
                    }
                }
                impl confkit::config_file::Config for #name {
                    fn load() -> Result<Self, confkit::error_handling::ConfigError> {
                        confkit::config_file::load(
                            &#dir_name,
                            &#file_name
                        )
                    }
                    
                    fn comments() -> Vec<::confkit::private::Comment> {
                        vec![#(#comments,)*]
                    }
                    
                    fn generate_config_file() -> Result<String, ::confkit::error_handling::ConfigError> {
                        let helper = #helper_name::default();
                        let config_contents = ::confkit::private::toml_edit::ser::to_string_pretty(&helper).unwrap();
                        
                        let comment_data = Self::comments();
                        
                        confkit::config_file::config_generation::generate_config_file(
                            config_contents,
                            comment_data
                         )
                    }
                    
                    fn init() -> Result<Self, ::confkit::error_handling::ConfigError> {
                        let helper = #helper_name::default();
                        let config_contents = ::confkit::private::toml_edit::ser::to_string_pretty(&helper).unwrap();
                        
                        let comment_data = Self::comments();
                        
                        ::confkit::config_file::init(
                            &#dir_name,
                            &#file_name,
                            config_contents,
                            comment_data
                        )
                    }
                }    
            )); 
        }
    }
    
    // This will only run if the above if let block fails to match
    // its purpose is to stop code from compiling if the user attempts to use the derive on a non struct
    TokenStream::from(syn::Error::new(
        input.ident.span(),
        "Only structs are supported by confkit derive",
    ).to_compile_error())
}

fn parse_field_attrs(fields :&syn::FieldsNamed) -> Result<Vec<FieldInfo>, syn::Error> {
    let mut errors :Option<syn::Error> = None;
    let mut output :Vec<FieldInfo> = Vec::new();
    
    for field in &fields.named {
        let mut field_info = FieldInfo {
            name: field.ident.clone().unwrap(),
            ty: field.ty.clone(),
            attrs: FieldAttrs {
                comment: None,
                default: None,
                nested: false,
            }
        };
        
        let mut found_default = false;
        let mut found_comment = false;
        let mut found_nested = false;
        
        for attr in &field.attrs {
            if !attr.path().is_ident("detail") {
                add_error(&mut errors, syn::Error::new_spanned(
                    attr, "unknown attribute, expected 'detail'"
                ));
                continue
            }
            
            let metadata = {
                match attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated) {
                    Ok(value) => value,
                    Err(error) => {
                        add_error(&mut errors, syn::Error::new_spanned(attr, &error.to_string()));
                        continue;
                    },
                }
            };
                        
            for meta in metadata {
                match &meta {
                    Meta::Path(name) if name.is_ident("nested") => {
                        if found_nested {
                            add_error(&mut errors, syn::Error::new_spanned(
                            &meta, "found redundant use of the 'nested' option"
                            ))
                        } else {
                            found_nested = true;
                            field_info.attrs.nested = true; 
                        }                           
                    },
                    Meta::List(name) if name.path.is_ident("comment") => {
                        if found_comment {
                            add_error(&mut errors, syn::Error::new_spanned(
                            &meta, "only one 'comment' option can be specified per field"
                            ))
                        } else {
                            found_comment = true;
                        
                            match name.parse_args::<LitStr>() {
                                Ok(comment) => field_info.attrs.comment = Some(comment.value()),
                                Err(error) => {
                                    add_error(&mut errors, error);
                                    continue;    
                                },
                            };                        
                        }
                    },
                    Meta::NameValue(name) if name.path.is_ident("default") => {
                        if found_default {
                            add_error(&mut errors, syn::Error::new_spanned(
                            &meta, "only one 'default' option can be specified per field"
                            ))
                        } else {
                            found_default = true;
                            field_info.attrs.default = Some(name.value.clone());
                        }
                    },
                    _ => {
                        add_error(&mut errors, syn::Error::new_spanned(
                        &meta, "unknown detail option, expected `default`, `comment', or 'nested'"
                        )) 
                    },
                }
            }
        }
        output.push(field_info);
    }
    
    match errors {
        Some(error) => Err(error),
        None => Ok(output),
    }
}

fn parse_struct_attrs(attrs :&[syn::Attribute]) -> Result<StructAttrs, syn::Error> {
    let mut found_config_attr = false;
    let mut found_dir_meta = false;
    let mut found_file_meta = false;
    
    let mut output = StructAttrs {
        dir_name: None,
        file_name: None,
    };
    
    let mut errors :Option<syn::Error> = None;
    
    for attr in attrs {
        if !attr.path().is_ident("config") {
            continue
        }
        
        if found_config_attr {
            add_error(
                &mut errors,syn::Error::new_spanned(attr, "config attribute can only be used once")
            )
        }
        
        found_config_attr = true;
        
        let metadata = {
            attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
        }.map_err(|e| syn::Error::new_spanned(attr, &e.to_string()))?;
        
        for meta in metadata {
            match &meta {
                Meta::NameValue(name) if name.path.is_ident("dir") => {
                    if found_dir_meta {
                        add_error(&mut errors, syn::Error::new_spanned(
                            &meta, "config dir name can only be specified once"
                        ))
                    }  
                    
                    found_dir_meta = true;
                    
                    let name = parse_string_expr(name.value.clone(), meta);
                    match name {
                        Ok(name) => output.dir_name = Some(name),
                        Err(e) => add_error(&mut errors, e),
                    }  
                    
                },
                Meta::NameValue(name) if name.path.is_ident("file") => {
                    if found_file_meta {
                        add_error(&mut errors, syn::Error::new_spanned(
                            &meta, "config file name can only be specified once"
                        ))
                    }  
                    
                    found_file_meta = true;
                    
                    let name = parse_string_expr(name.value.clone(), meta);
                    match name {
                        Ok(name) => output.file_name = Some(name),
                        Err(e) => add_error(&mut errors, e),
                    }                    
                },
                _ => {
                    add_error(&mut errors, syn::Error::new_spanned(
                    &meta, "unknown config option, expected `dir` or `file`"
                    ))
                },
            }
        }
    };
    
    match errors {
        Some(error) => Err(error),
        None => Ok(output),
    }
}

fn parse_string_expr<T>(input_expr :syn::Expr, span :T) -> Result<String, syn::Error> where T:ToTokens {
    match input_expr {
        syn::Expr::Lit(expr) => match expr.lit {
            syn::Lit::Str(value) => Ok(value.value()),
            _ => return Err(syn::Error::new_spanned(span, "value must be a string")),
        }
        _ => return Err(syn::Error::new_spanned(span, "value must be a string")),
    }
}

fn add_error(errors: &mut Option<syn::Error>, new_error :syn::Error) {
    match errors {
        Some(error) => error.combine(new_error),
        None => *errors = Some(new_error),
    }
}