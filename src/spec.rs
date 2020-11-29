use std::collections::BTreeMap;
use std::slice::Iter;

use openapi::v3_0 as orig;
use openapi::v3_0::PathItem;
use proc_macro2::{Punct, Spacing, TokenStream};
use quote::{quote, TokenStreamExt, ToTokens};

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

impl From<orig::PathItem> for Operations {
    fn from(path_item: PathItem) -> Self {
        let operations: Vec<Operation> = vec![
            (Method::Get, path_item.get),
            (Method::Delete, path_item.delete),
            (Method::Head, path_item.head),
            (Method::Options, path_item.options),
            (Method::Patch, path_item.patch),
            (Method::Post, path_item.post),
            (Method::Put, path_item.put),
        ]
            .into_iter()
            .filter_map(|(m, op)| match op {
                None => None,
                Some(o) => Some(Operation::from((m, o))),
            })
            .collect();
        Self(operations)
    }
}

type OperationId = String;

#[derive(Debug, Eq, PartialEq)]
pub(crate) enum Method {
    Get,
    Delete,
    Head,
    Options,
    Patch,
    Post,
    Put,
}

#[derive(Eq, PartialEq, Debug)]
pub struct Operation {
    pub id: OperationId,
    pub(crate) method: Method,
}

impl From<(Method, orig::Operation)> for Operation {
    fn from((method, op): (Method, orig::Operation)) -> Self {
        let id: OperationId = op.operation_id.expect("Operation Id is required");
        Self { method, id }
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
