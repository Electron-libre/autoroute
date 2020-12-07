use proc_macro::TokenStream;

use proc_macro_error::{abort, abort_call_site, proc_macro_error};
use syn::{parse_macro_input, LitStr};

mod actix_adapter;
#[allow(unused)]
mod spec;

const HANDLER_EXTENSION_NAME: &str = "x-autoroute-handler";
const RESOURCE_EXTENSION: &str = "x-autoroute-resource";

fn gen_config(spec: openapi::OpenApi) -> TokenStream {
    if let openapi::OpenApi::V3_0(spec) = spec {
        let spec = spec::Spec::from(spec);
        actix_adapter::gen_config(spec).into()
    } else {
        abort_call_site!("Autoroute support only V3 specification")
    }
}

/// Load open api specification from given file path.
/// Configure the web framework according to the specification
#[proc_macro_error]
#[proc_macro]
pub fn gen_config_from_path(input: TokenStream) -> TokenStream {
    let file = parse_macro_input!(input as LitStr);
    let spec_result = openapi::from_path(file.value().as_str());
    match spec_result {
        Ok(spec) => gen_config(spec),
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
    let spec_result = openapi::from_reader(spec_reader.value().as_bytes());
    match spec_result {
        Ok(spec) => gen_config(spec),
        Err(err) => abort!(
            spec_reader.span(),
            format!("Failed to parse OpenAPI specification: {:?}", err)
        ),
    }
}
