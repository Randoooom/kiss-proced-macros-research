/*
 * Copyright (C) 2022 Fritz Ochsmann
 *
 * This program is free software: you can redistribute it and/or modify
 * it under the terms of the GNU Affero General Public License as published
 * by the Free Software Foundation, either version 3 of the License, or
 * (at your option) any later version.
 *
 * This program is distributed in the hope that it will be useful,
 * but WITHOUT ANY WARRANTY; without even the implied warranty of
 * MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 * GNU Affero General Public License for more details.
 *
 * You should have received a copy of the GNU Affero General Public License
 * along with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

extern crate darling;
extern crate proc_macro;
#[macro_use]
extern crate syn;
#[macro_use]
extern crate quote;

use proc_macro::TokenStream;

use darling::FromMeta;
use proc_macro2::Ident;
use syn::{Data, DeriveInput, Field, GenericArgument, Path, PathArguments, Type};

#[derive(Debug, FromMeta, Clone)]
struct ReverseFlatOptions {
    prefix: String,
}

#[derive(Debug)]
struct TargetField {
    field: Field,
    prefix: String,
}

#[proc_macro_derive(ReverseFlat, attributes(reverse_flat))]
pub fn reverse_flat_macro_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // parse the data
    let data = match input.data {
        Data::Struct(data) => data,
        _ => panic!("Expected struct"),
    };

    let mut normal_fields: Vec<Field> = Vec::new();
    let mut target_usage: Vec<proc_macro2::TokenStream> = Vec::new();

    // iter through the given struct
    data.fields.into_iter().for_each(|field| {
        // get the given attributes of the field
        let attributes = &field.attrs;

        match attributes.into_iter().find_map(|attribute| {
            match ReverseFlatOptions::from_meta(&attribute.parse_meta().unwrap()) {
                Ok(options) => Some(TargetField {
                    prefix: options.prefix,
                    field: field.clone(),
                }),
                Err(_) => None,
            }
        }) {
            Some(target) => {
                let path = match target.field.ty {
                    Type::Path(typath) => typath.path,
                    _ => panic!("Expected path"),
                };

                let usage = impl_target(target.field.ident.as_ref().unwrap(), target.prefix, path);
                target_usage.push(usage);
            }
            None => normal_fields.push(field),
        };
    });

    // process the normal data
    let normal_ident = format_ident!("__Normal{}", name);
    let (normal_declaration, normal_idents) = impl_normal_declaration(normal_fields, &normal_ident);

    let expanded = quote! {
        // include the normal declaration here
        #normal_declaration

        impl ReverseFlat for #name {
            fn reverse(value: serde_json::Value) -> std::result::Result<Self, serde_json::Error> {
                // parse here the normal root object
                let root = serde_json::from_value::<#normal_ident>(value.clone())?;

                // Now we got all of our parts together and "just" need to build the final object
                Ok(
                    Self {
                        #(#target_usage)*
                        #(#normal_idents: root.#normal_idents,)*
                    }
                )
            }
        }
    };

    expanded.into()
}

fn impl_normal_declaration(
    fields: Vec<Field>,
    name: &Ident,
) -> (proc_macro2::TokenStream, Vec<Ident>) {
    // separate the ident and the type
    let idents = fields
        .iter()
        .map(|field| field.ident.clone().unwrap())
        .collect::<Vec<Ident>>();
    let paths = fields
        .iter()
        .map(|field| match &field.ty {
            syn::Type::Path(ty_path) => &ty_path.path,
            _ => panic!("invalid type"),
        })
        .collect::<Vec<&Path>>();

    let expanded = quote! {
        #[derive(Deserialize)]
        struct #name {
            #(#idents : #paths,)*
        }
    };

    (expanded.into(), idents)
}

fn impl_target(ident: &Ident, mut prefix: String, path: Path) -> proc_macro2::TokenStream {
    // push the '_' into the prefix
    prefix.push_str("_");

    // check if the target path is an option
    let is_option =
        path.segments.len() == 1 && path.segments.iter().next().unwrap().ident == "Option";
    let creation = if is_option {
        // parse the generic type from the option
        let type_params = &path.segments.first().unwrap().arguments;
        let generic = match type_params {
            PathArguments::AngleBracketed(params) => match params.args.iter().next().unwrap() {
                GenericArgument::Type(ty) => ty,
                _ => panic!("Invalid option generic"),
            },
            _ => panic!("Missing option generic"),
        };

        quote! {
            match #generic::reverse(target) {
                Ok(value) => Some(value),
                Err(_) => None,
            }
        }
    } else {
        quote! {
            #path::reverse(target)?
        }
    };

    let expanded = quote! {
        #ident: {
            use serde::de::Error;

            // convert the serde value into a new object
            let target = match value.clone() {
                serde_json::Value::Object(mut map) => {
                    // take the ownership of the map and apply filter map on it
                    map = serde_json::Map::from_iter(map
                        .into_iter()
                        .filter_map(|(key, value)| {
                        return if key.starts_with(#prefix) {
                            Some((key.replacen(#prefix, "", 1) ,value))
                        } else {
                            None
                        }
                    }));

                    Ok(serde_json::Value::from(map))
                },
                _ => Err(serde_json::Error::custom("Expected object"))
            }?;

            #creation
        },
    };

    expanded.into()
}
