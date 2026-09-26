use proc_macro::TokenStream;

use proc_macro_error::{abort, abort_call_site, proc_macro_error};
use serde_json::Value;
use syn::{LitStr, parse_macro_input};

mod actix_adapter;
mod schema;
mod spec;

const HANDLER_EXTENSION_NAME: &str = "x-autoroute-handler";
const RESOURCE_EXTENSION: &str = "x-autoroute-resource";
const VALIDATE_EXTENSION: &str = "x-autoroute-validate";

fn gen_config(document: &Value) -> TokenStream {
    let is_v3 = document
        .get("openapi")
        .and_then(Value::as_str)
        .is_some_and(|version| version.starts_with("3."));
    if !is_v3 {
        abort_call_site!("Autoroute support only V3 specification")
    }
    let spec = spec::Spec::parse(document);
    actix_adapter::gen_config(spec).into()
}

/// Load open api specification from given file path.
/// Configure the web framework according to the specification
#[proc_macro_error]
#[proc_macro]
pub fn gen_config_from_path(input: TokenStream) -> TokenStream {
    let file = parse_macro_input!(input as LitStr);
    let source = match std::fs::read_to_string(file.value()) {
        Ok(source) => source,
        Err(err) => abort!(
            file.span(),
            format!("Failed to load OpenAPI specification: {:?}", err)
        ),
    };
    match serde_yaml::from_str::<Value>(&source) {
        Ok(document) => gen_config(&document),
        Err(err) => abort!(
            file.span(),
            format!("Failed to load OpenAPI specification: {:?}", err)
        ),
    }
}

#[proc_macro_error]
#[proc_macro]
pub fn gen_config_from(input: TokenStream) -> TokenStream {
    let spec_reader = parse_macro_input!(input as LitStr);
    match serde_yaml::from_str::<Value>(&spec_reader.value()) {
        Ok(document) => gen_config(&document),
        Err(err) => abort!(
            spec_reader.span(),
            format!("Failed to parse OpenAPI specification: {:?}", err)
        ),
    }
}
