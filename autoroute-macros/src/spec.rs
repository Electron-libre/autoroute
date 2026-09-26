use std::fmt::{self, Display, Formatter};

use http::Method as httpMethod;
use proc_macro_error::abort_call_site;
use serde_json::Value;

use crate::{
    HANDLER_EXTENSION_NAME, RESOURCE_EXTENSION, VALIDATE_EXTENSION,
    schema::{resolve_component, standalone},
};

#[derive(Eq, PartialEq, Debug)]
pub struct Spec {
    pub paths: Paths,
}

impl Spec {
    /// Build the specification from a parsed `OpenAPI` 3 document.
    pub(crate) fn parse(root: &Value) -> Self {
        let validate = validate_flag(root, "the specification");
        Self {
            paths: Paths::parse(root, validate),
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct Paths(pub(crate) Vec<Path>);

impl Paths {
    fn parse(root: &Value, validate: Option<bool>) -> Self {
        let paths = match root.get("paths") {
            Some(Value::Object(paths)) => paths
                .iter()
                .map(|(url, item)| Path::parse(root, url, item, validate))
                .collect(),
            _ => Vec::new(),
        };
        Self(paths)
    }
}

type ResourceName = String;

#[derive(Eq, PartialEq, Debug)]
pub struct Path {
    pub url: Url,
    pub operations: Operations,
    pub resource_name: Option<ResourceName>,
}

impl Path {
    fn parse(root: &Value, url: &str, item: &Value, validate: Option<bool>) -> Self {
        let validate = validate_flag(item, url).or(validate);
        let resource_name = match item.get(RESOURCE_EXTENSION) {
            Some(Value::String(resource_name)) => Some(resource_name.clone()),
            _ => None,
        };
        Self {
            url: url.to_string(),
            operations: Operations::parse(root, url, item, validate),
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

impl Operations {
    fn parse(root: &Value, url: &str, item: &Value, validate: Option<bool>) -> Self {
        let operations = [
            httpMethod::GET,
            httpMethod::DELETE,
            httpMethod::HEAD,
            httpMethod::OPTIONS,
            httpMethod::PATCH,
            httpMethod::POST,
            httpMethod::PUT,
            httpMethod::TRACE,
        ]
        .into_iter()
        .filter_map(|method| {
            let operation = item.get(method.as_str().to_lowercase())?;
            Some(Operation::parse(
                root,
                url,
                item,
                Method::from(method),
                operation,
                validate,
            ))
        })
        .collect();
        Self(operations)
    }
}

#[derive(Eq, PartialEq, Debug)]
pub(crate) struct HandlerIdentifier(pub(crate) String);

impl HandlerIdentifier {
    fn from_operation(operation: &Value, path: &str, method: &Method) -> Self {
        match operation.get(HANDLER_EXTENSION_NAME) {
            Some(Value::String(handler_ident)) => {
                if !is_valid_handler_identifier(handler_ident) {
                    abort_call_site!(format!(
                        "Invalid autoroute handler identifier {:?} for {} {}: must be a valid Rust identifier",
                        handler_ident, method, path
                    ));
                }
                Self(handler_ident.into())
            }
            other => abort_call_site!(format!(
                "Invalid autoroute handler value identifier for {} {}: expected a string for {}, got {:?}",
                method, path, HANDLER_EXTENSION_NAME, other
            )),
        }
    }
}

/// Whether `ident` can be used as a Rust identifier, i.e. can safely be
/// turned into a `proc_macro2::Ident` without panicking.
fn is_valid_handler_identifier(ident: &str) -> bool {
    syn::parse_str::<syn::Ident>(ident).is_ok()
}

/// Read the optional `x-autoroute-validate` extension of `object`.
fn validate_flag(object: &Value, context: &str) -> Option<bool> {
    match object.get(VALIDATE_EXTENSION)? {
        Value::Bool(flag) => Some(*flag),
        other => abort_call_site!(
            "Invalid {} value for {}: expected a boolean, got {:?}",
            VALIDATE_EXTENSION,
            context,
            other
        ),
    }
}

#[derive(Eq, PartialEq, Debug)]
pub struct Operation {
    pub(crate) method: Method,
    pub(crate) handler: HandlerIdentifier,
    /// Present only when validation is enabled for this operation.
    pub(crate) validation: Option<Validation>,
}

impl Operation {
    fn parse(
        root: &Value,
        url: &str,
        path_item: &Value,
        method: Method,
        operation: &Value,
        validate: Option<bool>,
    ) -> Self {
        let handler = HandlerIdentifier::from_operation(operation, url, &method);
        let context = format!("{method} {url}");
        let validate = validate_flag(operation, &context).or(validate);
        let validation = validate
            .unwrap_or(false)
            .then(|| Validation::parse(root, &context, path_item, operation));
        Self {
            method,
            handler,
            validation,
        }
    }
}

/// What has to be validated for an operation: its parameters and its body.
#[derive(Eq, PartialEq, Debug)]
pub(crate) struct Validation {
    pub(crate) parameters: Vec<Parameter>,
    pub(crate) body: Option<RequestBody>,
}

#[derive(Eq, PartialEq, Debug, Clone, Copy)]
pub(crate) enum ParameterLocation {
    Path,
    Query,
    Header,
    Cookie,
}

impl ParameterLocation {
    fn parse(location: &str, context: &str, name: &str) -> Self {
        match location {
            "path" => Self::Path,
            "query" => Self::Query,
            "header" => Self::Header,
            "cookie" => Self::Cookie,
            other => abort_call_site!(
                "Invalid location {:?} of parameter `{}` for {}",
                other,
                name,
                context
            ),
        }
    }
}

#[derive(Eq, PartialEq, Debug)]
pub(crate) struct Parameter {
    pub(crate) name: String,
    pub(crate) location: ParameterLocation,
    pub(crate) required: bool,
    pub(crate) style: Option<String>,
    pub(crate) explode: Option<bool>,
    /// Standalone JSON Schema of the parameter value.
    pub(crate) schema: Value,
}

#[derive(Eq, PartialEq, Debug)]
pub(crate) struct RequestBody {
    pub(crate) required: bool,
    /// Standalone JSON Schema of the `application/json` payload.
    pub(crate) schema: Value,
}

const JSON_MEDIA_TYPE: &str = "application/json";

impl Validation {
    fn parse(root: &Value, context: &str, path_item: &Value, operation: &Value) -> Self {
        Self {
            parameters: Parameter::parse_all(root, context, path_item, operation),
            body: RequestBody::parse(root, context, operation),
        }
    }
}

/// Compile `schema` to report an invalid one at compile time.
fn check_schema(schema: &Value, context: &str, what: &str) {
    if let Err(err) = jsonschema::draft4::new(schema) {
        abort_call_site!("Invalid schema of {} for {}: {}", what, context, err);
    }
}

/// Follow `$ref` until an actual object is found.
fn deref<'a>(root: &'a Value, kind: &str, object: &'a Value) -> &'a Value {
    match object.get("$ref") {
        Some(Value::String(reference)) => resolve_component(root, kind, reference),
        _ => object,
    }
}

impl Parameter {
    /// Parameters of the path item overridden by the ones of the operation.
    fn parse_all(root: &Value, context: &str, path_item: &Value, operation: &Value) -> Vec<Self> {
        let mut parameters: Vec<Self> = Vec::new();
        for declared in [path_item, operation] {
            let Some(Value::Array(declared)) = declared.get("parameters") else {
                continue;
            };
            for parameter in declared {
                let parameter = Self::parse(root, context, deref(root, "parameters", parameter));
                parameters
                    .retain(|p| (&p.name, p.location) != (&parameter.name, parameter.location));
                parameters.push(parameter);
            }
        }
        parameters
    }

    fn parse(root: &Value, context: &str, parameter: &Value) -> Self {
        let Some(name) = parameter.get("name").and_then(Value::as_str) else {
            abort_call_site!("Parameter without a name for {}", context);
        };
        let location = ParameterLocation::parse(
            parameter
                .get("in")
                .and_then(Value::as_str)
                .unwrap_or_default(),
            context,
            name,
        );
        let Some(schema) = parameter.get("schema") else {
            abort_call_site!(
                "Parameter `{}` for {} has no `schema`: only schema based parameters can be validated",
                name,
                context
            );
        };
        let schema = standalone(root, schema);
        check_schema(&schema, context, &format!("parameter `{name}`"));
        Self {
            name: name.to_string(),
            location,
            required: location == ParameterLocation::Path
                || parameter.get("required") == Some(&Value::Bool(true)),
            style: parameter
                .get("style")
                .and_then(Value::as_str)
                .map(str::to_string),
            explode: parameter.get("explode").and_then(Value::as_bool),
            schema,
        }
    }
}

impl RequestBody {
    fn parse(root: &Value, context: &str, operation: &Value) -> Option<Self> {
        let body = deref(root, "requestBodies", operation.get("requestBody")?);
        let Some(media_type) = body.get("content").and_then(|c| c.get(JSON_MEDIA_TYPE)) else {
            abort_call_site!(
                "Request body of {} has no `{}` content: only JSON bodies can be validated \
                 (set {}: false to skip the validation of this operation)",
                context,
                JSON_MEDIA_TYPE,
                VALIDATE_EXTENSION
            );
        };
        let schema = standalone(root, media_type.get("schema")?);
        check_schema(&schema, context, "request body");
        Some(Self {
            required: body.get("required") == Some(&Value::Bool(true)),
            schema,
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn document(yaml: &str) -> Value {
        serde_yaml::from_str(yaml).expect("valid YAML document")
    }

    #[test]
    fn test_spec_from_empty_document() {
        let spec = Spec::parse(&document("openapi: 3.0.0\npaths: {}\n"));
        assert_eq!(
            spec,
            Spec {
                paths: Paths(vec![])
            }
        );
    }

    #[test]
    fn test_handler_identifier_from_valid_identifier() {
        let operation = json!({"x-autoroute-handler": "test_handler"});
        let method = Method::from(httpMethod::GET);
        assert_eq!(
            HandlerIdentifier::from_operation(&operation, "/test", &method),
            HandlerIdentifier("test_handler".to_string())
        );
    }

    // `HandlerIdentifier::from_operation` aborts (via `abort_call_site!`) on an invalid
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

    const VALIDATED_API: &str = r##"
openapi: 3.0.0
x-autoroute-validate: true
paths:
  /foo/{fooId}:
    parameters:
      - name: fooId
        in: path
        schema: {type: integer, maximum: 10}
      - $ref: "#/components/parameters/Limit"
    get:
      x-autoroute-handler: get_foo
      parameters:
        - name: limit
          in: query
          schema: {type: integer, minimum: 5}
    post:
      x-autoroute-handler: add_foo
      x-autoroute-validate: false
    put:
      x-autoroute-handler: set_foo
      requestBody:
        required: true
        content:
          application/json:
            schema: {$ref: "#/components/schemas/Foo"}
components:
  parameters:
    Limit:
      name: limit
      in: query
      schema: {type: integer}
  schemas:
    Foo:
      type: object
      required: [name]
      properties:
        name: {type: string, pattern: "^a"}
"##;

    fn operation<'a>(spec: &'a Spec, method: &httpMethod) -> &'a Operation {
        spec.paths.0[0]
            .operations
            .0
            .iter()
            .find(|op| op.method.0 == *method)
            .expect("operation present")
    }

    #[test]
    fn test_parameters_merge_with_operation_override() {
        let spec = Spec::parse(&document(VALIDATED_API));
        let validation = operation(&spec, &httpMethod::GET)
            .validation
            .as_ref()
            .expect("validation enabled by the specification");

        let names: Vec<_> = validation.parameters.iter().map(|p| &p.name[..]).collect();
        assert_eq!(names, ["fooId", "limit"]);
        assert!(validation.parameters[0].required);
        assert_eq!(validation.parameters[0].location, ParameterLocation::Path);
        assert_eq!(
            validation.parameters[1].schema,
            json!({"type": "integer", "minimum": 5})
        );
        assert!(validation.body.is_none());
    }

    #[test]
    fn test_operation_can_opt_out() {
        let spec = Spec::parse(&document(VALIDATED_API));
        assert!(operation(&spec, &httpMethod::POST).validation.is_none());
    }

    #[test]
    fn test_request_body_schema_is_standalone() {
        let spec = Spec::parse(&document(VALIDATED_API));
        let body = operation(&spec, &httpMethod::PUT)
            .validation
            .as_ref()
            .and_then(|v| v.body.as_ref())
            .expect("request body");

        assert!(body.required);
        assert!(
            body.schema
                .pointer("/components/schemas/Foo/properties/name/pattern")
                .is_some()
        );
    }

    #[test]
    fn test_validation_is_off_by_default() {
        let spec = Spec::parse(&document(
            "openapi: 3.0.0\npaths:\n  /foo:\n    get:\n      x-autoroute-handler: h\n",
        ));
        assert!(operation(&spec, &httpMethod::GET).validation.is_none());
    }

    #[test]
    fn test_document_with_numeric_response_codes_and_schema_constraints() {
        let spec = Spec::parse(&document(
            "openapi: 3.0.0\npaths:\n  /foo:\n    get:\n      x-autoroute-handler: h\n      \
             parameters:\n        - {name: n, in: query, schema: {type: integer, maximum: 10, exclusiveMaximum: true}}\n      \
             responses:\n        200:\n          description: ok\n",
        ));
        assert_eq!(spec.paths.0.len(), 1);
    }
}
