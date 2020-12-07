use std::{
    collections::BTreeMap,
    fmt::{self, Display, Formatter},
    slice::Iter,
};

use http::Method as httpMethod;
use openapi::{
    v3_0 as orig,
    v3_0::{PathItem, Value},
};
use proc_macro2::{Punct, Spacing, TokenStream};
use proc_macro_error::abort_call_site;
use quote::{quote, ToTokens, TokenStreamExt};

use crate::HANDLER_EXTENSION_NAME;

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

#[derive(Eq, PartialEq, Debug)]
pub struct Path {
    pub url: Url,
    pub operations: Operations,
}

impl From<(String, orig::PathItem)> for Path {
    fn from((url, path_item): (String, PathItem)) -> Self {
        let operations = Operations::from(path_item);
        Self { url, operations }
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

impl From<orig::PathItem> for Operations {
    fn from(path_item: PathItem) -> Self {
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
        .filter_map(|(m, op)| match op {
            None => None,
            Some(o) => Some(Operation::from((Method::from(m), o))),
        })
        .collect();
        Self(operations)
    }
}

type OperationId = String;

#[derive(Eq, PartialEq, Debug)]
pub(crate) struct HandlerIdentifier(pub(crate) String);

impl HandlerIdentifier {}

impl From<orig::Extensions> for HandlerIdentifier {
    fn from(extensions: orig::Extensions) -> Self {
        match extensions.get(HANDLER_EXTENSION_NAME) {
            // TODO validate handler format as identifier
            Some(Value::String(handler_ident)) => Self(handler_ident.into()),
            _ => abort_call_site!(format!(
                "Invalid autoroute handler value identifier: {:?}",
                extensions
            )),
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct Operation {
    pub(crate) id: Option<OperationId>,
    pub(crate) method: Method,
    pub(crate) handler: HandlerIdentifier,
}

impl From<(Method, orig::Operation)> for Operation {
    fn from((method, op): (Method, orig::Operation)) -> Self {
        let id = op.operation_id.to_owned();
        let handler = HandlerIdentifier::from(op.extensions);
        Self {
            method,
            id,
            handler,
        }
    }
}

#[cfg(test)]
mod tests {
    use openapi::v3_0 as orig;

    use super::*;

    #[test]
    fn test_spec_from_orig() {
        let orig_spec = orig::Spec {
            openapi: "".to_string(),
            info: Default::default(),
            servers: None,
            paths: Default::default(),
            components: None,
            tags: None,
            external_docs: None,
            extensions: Default::default(),
        };

        let spec = Spec::from(orig_spec);
        assert_eq!(
            spec,
            Spec {
                paths: Paths(vec![])
            }
        )
    }
}
