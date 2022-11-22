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
use syn::{Data, DeriveInput, Field, Path, Type};

#[derive(FromMeta)]
struct ReverseFlatOptions {
    prefix: String,
}

struct TargetField {
    field: Field,
    prefix: String,
}

/// This creates the 'normal' declaration of the target struct.
/// It contains the definition of all the root level fields contained by it.
///
/// *fields* equals the `Vec<Field>` of all root level fields.
/// *name* equals the new ident for the struct.
fn impl_normal_declaration(
    fields: Vec<Field>,
    name: &Ident,
) -> (proc_macro2::TokenStream, Vec<Ident>) {
    // this creates the raw field definition for all the normal root fields in the struct.
    let (field_blocks, idents) = fields
        .into_iter()
        .map(|field| {
            let ident = field.ident.unwrap();
            // extract the type of the field as a `Path`
            let path = match field.ty {
                Type::Path(ty_path) => ty_path.path,
                _ => panic!("invalid type"),
            };

            (
                quote! {
                    #ident: #path
                }
                .into(),
                ident,
            )
        })
        .unzip::<_, _, Vec<proc_macro2::TokenStream>, Vec<Ident>>();

    // Here we expand the root part of the reversible struct,
    // by creating a new struct containing all the root fields which
    // do not need to be reversed by the macro.
    let declaration = quote! {
        #[derive(Deserialize)]
        struct #name {
            #(#field_blocks,)*
        }
    };

    (declaration.into(), idents)
}

/// This function implements the reverse transformation process for a target field
fn impl_target(ident: &Ident, mut prefix: String, path: Path) -> proc_macro2::TokenStream {
    // push the '_' into the prefix, because a prefix has to end with it.
    prefix.push_str("_");

    let expanded = quote! {
        #ident: {
            // modify the cloned instance of the target_map iterator for
            // removing the given prefix from all fields of the child struct
            let map = serde_json::Map::from_iter(
                target_map
                .clone()
                .into_iter()
                .filter_map(|(key, value)| {
                    return if key.starts_with(#prefix) {
                        Some((key.replacen(#prefix, "", 1) ,value))
                    } else {
                        None
                    }
            }));
            // convert the map into a `serde_json::Value`
            let value = serde_json::Value::from(map);

            #path::reverse(value)?
        },
    };

    expanded.into()
}

#[proc_macro_derive(ReverseFlat, attributes(reverse))]
pub fn reverse_flat_macro_derive(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // match the enum type of the data and only accept the `DataStruct`
    // otherwise the macro will cause a panic.
    let data = match input.data {
        Data::Struct(data) => data,
        _ => panic!("Expected struct"),
    };

    // create empty vectors for storing data of the targets and the normal fields
    let mut normal_fields: Vec<Field> = Vec::new();
    let mut target_usage: Vec<proc_macro2::TokenStream> = Vec::new();

    // iter through all fields of the parsed `DataStruct`
    data.fields.into_iter().for_each(|field| {
        // access the given attributes of the field
        let attributes = &field.attrs;

        // iter through them and try to find an attribute ( / only the first) matching an instance
        // of the `ReverseFlatOption` struct.
        match attributes.into_iter().find_map(|attribute| {
            match ReverseFlatOptions::from_meta(&attribute.parse_meta().unwrap()) {
                // if the attribute matches the struct, return a build `TargetField`.
                // Otherwise the `find_map` will skip the attribute
                Ok(options) => Some(TargetField {
                    prefix: options.prefix,
                    field: field.clone(),
                }),
                Err(_) => None,
            }
        }) {
            Some(target) => {
                // extract the path of the target
                let path = match target.field.ty {
                    Type::Path(type_path) => type_path.path,
                    _ => panic!("Expected path"),
                };

                // expand the usage for the specific target
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
                use serde::de::Error;

                // parse here the normal root object
                let root = serde_json::from_value::<#normal_ident>(value.clone())?;
                // gain a `serde_json::Value::Object` out of the given value and extract the map
                let target_map = match value {
                    serde_json::Value::Object(map) => Ok(map),
                    _ => Err(serde_json::Error::custom("Invalid type: expected object")),
                }?;

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
