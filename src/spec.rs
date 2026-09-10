use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter},
};

use http::Method as httpMethod;
use openapi::{
    v3_0 as orig,
    v3_0::{PathItem, Value},
};
use proc_macro_error::abort_call_site;

use crate::{HANDLER_EXTENSION_NAME, RESOURCE_EXTENSION};

#[derive(Eq, PartialEq, Debug)]
pub struct Spec {
    pub paths: Paths,
}

impl From<orig::Spec> for Spec {
    fn from(spec: orig::Spec) -> Self {
        Self {
            paths: Paths::from(spec.paths),
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct Paths(pub(crate) Vec<Path>);

impl From<BTreeMap<String, orig::PathItem>> for Paths {
    fn from(paths: BTreeMap<String, PathItem>) -> Self {
        Self(paths.into_iter().map(Path::from).collect())
    }
}

type ResourceName = String;

#[derive(Eq, PartialEq, Debug)]
pub struct Path {
    pub url: Url,
    pub operations: Operations,
    pub resource_name: Option<ResourceName>,
}

impl From<(String, orig::PathItem)> for Path {
    fn from((url, path_item): (String, PathItem)) -> Self {
        let operations = Operations::from((url.clone(), path_item.clone()));
        let resource_name = match path_item.extensions.get(RESOURCE_EXTENSION) {
            Some(Value::String(resource_name)) => Some(resource_name.to_owned()),
            _ => None,
        };
        Self {
            url,
            operations,
            resource_name,
        }
    }
}

type Url = String;

#[derive(Eq, PartialEq, Debug)]
pub struct Operations(pub(crate) Vec<Operation>);

#[derive(Debug, Eq, PartialEq)]
pub struct Method(pub(crate) httpMethod);

impl Display for Method {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0)
    }
}

impl From<httpMethod> for Method {
    fn from(http_method: httpMethod) -> Self {
        Self(http_method)
    }
}

impl From<(Url, orig::PathItem)> for Operations {
    fn from((url, path_item): (Url, PathItem)) -> Self {
        let operations: Vec<Operation> = vec![
            (httpMethod::GET, path_item.get),
            (httpMethod::DELETE, path_item.delete),
            (httpMethod::HEAD, path_item.head),
            (httpMethod::OPTIONS, path_item.options),
            (httpMethod::PATCH, path_item.patch),
            (httpMethod::POST, path_item.post),
            (httpMethod::PUT, path_item.put),
            (httpMethod::TRACE, path_item.trace),
        ]
        .into_iter()
        .filter_map(|(m, op)| op.map(|o| Operation::from((url.clone(), Method::from(m), o))))
        .collect();
        Self(operations)
    }
}

#[derive(Eq, PartialEq, Debug)]
pub(crate) struct HandlerIdentifier(pub(crate) String);

impl HandlerIdentifier {
    fn from_extensions(extensions: &orig::Extensions, path: &str, method: &Method) -> Self {
        match extensions.get(HANDLER_EXTENSION_NAME) {
            Some(Value::String(handler_ident)) => {
                if !is_valid_handler_identifier(handler_ident) {
                    abort_call_site!(format!(
                        "Invalid autoroute handler identifier {:?} for {} {}: must be a valid Rust identifier",
                        handler_ident, method, path
                    ));
                }
                Self(handler_ident.into())
            }
            _ => abort_call_site!(format!(
                "Invalid autoroute handler value identifier for {} {}: {:?}",
                method, path, extensions
            )),
        }
    }
}

/// Whether `ident` can be used as a Rust identifier, i.e. can safely be
/// turned into a `proc_macro2::Ident` without panicking.
fn is_valid_handler_identifier(ident: &str) -> bool {
    syn::parse_str::<syn::Ident>(ident).is_ok()
}

#[derive(Eq, PartialEq, Debug)]
pub struct Operation {
    pub(crate) method: Method,
    pub(crate) handler: HandlerIdentifier,
}

impl From<(Url, Method, orig::Operation)> for Operation {
    fn from((url, method, op): (Url, Method, orig::Operation)) -> Self {
        let handler = HandlerIdentifier::from_extensions(&op.extensions, &url, &method);
        Self { method, handler }
    }
}

#[cfg(test)]
mod tests {
    use openapi::v3_0 as orig;

    use super::*;

    #[test]
    fn test_spec_from_orig() {
        let orig_spec = orig::Spec {
            openapi: String::new(),
            info: orig::Info::default(),
            servers: None,
            paths: BTreeMap::default(),
            components: None,
            tags: None,
            external_docs: None,
            extensions: orig::Extensions::default(),
        };

        let spec = Spec::from(orig_spec);
        assert_eq!(
            spec,
            Spec {
                paths: Paths(vec![])
            }
        );
    }

    /// Build the `x-autoroute-handler` extensions map for a single GET
    /// operation, the same way it would be produced by parsing a real
    /// `OpenAPI` document.
    fn extensions_with_handler(handler: &str) -> orig::Extensions {
        let yaml = format!(
            "openapi: \"3.0.0\"\n\
             info:\n  title: test\n  version: \"1\"\n\
             paths:\n  /test:\n    get:\n      responses:\n        \"200\":\n          description: success\n      x-autoroute-handler: \"{handler}\"\n"
        );
        match openapi::from_reader(yaml.as_bytes()).expect("valid OpenAPI document") {
            openapi::OpenApi::V3_0(spec) => spec
                .paths
                .get("/test")
                .expect("path present")
                .get
                .as_ref()
                .expect("operation present")
                .extensions
                .clone(),
            openapi::OpenApi::V2(_) => panic!("expected a V3 specification"),
        }
    }

    #[test]
    fn test_handler_identifier_from_valid_identifier() {
        let extensions = extensions_with_handler("test_handler");
        let method = Method::from(httpMethod::GET);
        assert_eq!(
            HandlerIdentifier::from_extensions(&extensions, "/test", &method),
            HandlerIdentifier("test_handler".to_string())
        );
    }

    // `HandlerIdentifier::from_extensions` aborts (via `abort_call_site!`) on an invalid
    // identifier, which only unwinds cleanly from within a real proc-macro
    // `entry_point`. So the validation predicate it relies on is exercised
    // directly here instead of trying to catch the abort from a plain unit
    // test — this is what used to let an invalid `x-autoroute-handler` value
    // reach `Ident::new` and panic deep inside `actix_adapter.rs`.
    #[test]
    fn test_is_valid_handler_identifier() {
        assert!(is_valid_handler_identifier("test_handler"));
        assert!(is_valid_handler_identifier("_leading_underscore"));

        assert!(!is_valid_handler_identifier(""));
        assert!(!is_valid_handler_identifier("not a valid ident!"));
        assert!(!is_valid_handler_identifier("123abc"));
        assert!(!is_valid_handler_identifier("path::to::handler"));
        assert!(!is_valid_handler_identifier("self"));
    }
}
