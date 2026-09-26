//! Validation of the requests against the JSON schemas of the specification.
//!
//! The types of this module are targeted by the code generated when an
//! operation is flagged with `x-autoroute-validate: true`.

use std::{
    future::{Ready, ready},
    pin::Pin,
    rc::Rc,
    sync::Arc,
};

use actix_web::{
    Error, FromRequest, HttpMessage, HttpRequest, HttpResponse,
    body::EitherBody,
    dev::{Payload, Service, ServiceRequest, ServiceResponse, Transform, forward_ready},
    http::header::{CONTENT_TYPE, HeaderValue},
    web::Bytes,
};
use jsonschema::Validator;
use serde_json::{Number, Value, json};

const SCHEMA_REF_PREFIX: &str = "#/components/schemas/";
/// Bound on the `$ref` chain followed to find the type of a schema.
const MAX_REF_DEPTH: usize = 32;

/// Where a parameter is read from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Location {
    Path,
    Query,
    Header,
    Cookie,
}

impl Location {
    fn as_str(self) -> &'static str {
        match self {
            Self::Path => "path",
            Self::Query => "query",
            Self::Header => "header",
            Self::Cookie => "cookie",
        }
    }
}

/// A parameter to validate, as declared by the specification.
pub struct ParameterSpec {
    name: &'static str,
    location: Location,
    required: bool,
    style: Option<&'static str>,
    explode: Option<bool>,
    schema: &'static str,
}

impl ParameterSpec {
    /// `schema` is the standalone JSON schema of the parameter value.
    #[must_use]
    pub fn new(
        name: &'static str,
        location: Location,
        required: bool,
        style: Option<&'static str>,
        explode: Option<bool>,
        schema: &'static str,
    ) -> Self {
        Self {
            name,
            location,
            required,
            style,
            explode,
            schema,
        }
    }
}

/// The JSON body to validate, as declared by the specification.
pub struct BodySpec {
    required: bool,
    schema: &'static str,
}

impl BodySpec {
    /// `schema` is the standalone JSON schema of the payload.
    #[must_use]
    pub fn new(required: bool, schema: &'static str) -> Self {
        Self { required, schema }
    }
}

/// What to validate for the requests of one HTTP method.
pub struct OperationValidation {
    method: &'static str,
    parameters: Vec<ParameterSpec>,
    body: Option<BodySpec>,
}

impl OperationValidation {
    #[must_use]
    pub fn new(method: &'static str, parameters: Vec<ParameterSpec>) -> Self {
        Self {
            method,
            parameters,
            body: None,
        }
    }

    /// Also validate the JSON payload of the requests.
    #[must_use]
    pub fn with_body(mut self, body: BodySpec) -> Self {
        self.body = Some(body);
        self
    }
}

struct Parameter {
    spec: ParameterSpec,
    schema: Value,
    validator: Validator,
}

struct Body {
    required: bool,
    validator: Validator,
}

struct Operation {
    method: &'static str,
    parameters: Vec<Parameter>,
    body: Option<Body>,
}

/// A problem found in a request.
struct Problem {
    /// Where the problem lies: a parameter `Location`, or the `body`.
    location: &'static str,
    name: &'static str,
    pointer: String,
    message: String,
}

/// Actix middleware rejecting, with a `400 Bad Request`, the requests that
/// do not comply with the specification.
#[derive(Clone)]
pub struct Validation {
    operations: Arc<Vec<Operation>>,
}

impl Validation {
    /// # Panics
    ///
    /// When a schema is not valid. The macro already checked them at
    /// compile time, so this only happens with hand written specifications.
    #[must_use]
    pub fn new(operations: Vec<OperationValidation>) -> Self {
        let compile = |spec: ParameterSpec| {
            let schema: Value =
                serde_json::from_str(spec.schema).expect("parameter schema is valid JSON");
            let validator =
                jsonschema::draft4::new(&schema).expect("parameter schema is a valid JSON schema");
            Parameter {
                spec,
                schema,
                validator,
            }
        };
        let operations = operations
            .into_iter()
            .map(|operation| Operation {
                method: operation.method,
                parameters: operation.parameters.into_iter().map(compile).collect(),
                body: operation.body.map(|body| {
                    let schema: Value =
                        serde_json::from_str(body.schema).expect("body schema is valid JSON");
                    Body {
                        required: body.required,
                        validator: jsonschema::draft4::new(&schema)
                            .expect("body schema is a valid JSON schema"),
                    }
                }),
            })
            .collect();
        Self {
            operations: Arc::new(operations),
        }
    }

    fn operation(&self, method: &str) -> Option<&Operation> {
        self.operations
            .iter()
            .find(|operation| operation.method == method)
    }
}

/// The result of the validation of a payload.
enum BodyOutcome {
    Checked(Vec<Problem>),
    UnsupportedMediaType,
}

/// Whether the `Content-Type` is `application/json`, or a `+json` flavour of it.
fn is_json(content_type: Option<&HeaderValue>) -> bool {
    let Some(content_type) = content_type.and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let media_type = content_type
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    media_type == "application/json"
        || (media_type.starts_with("application/") && media_type.ends_with("+json"))
}

impl Body {
    /// Validate the payload of `req`, which is then made available again for
    /// the handler.
    ///
    /// The size of the payload is bounded by the `PayloadConfig` of the
    /// application.
    async fn check(&self, req: &mut ServiceRequest) -> Result<BodyOutcome, Error> {
        let http_request = req.request().clone();
        let mut payload = req.take_payload();
        let bytes = Bytes::from_request(&http_request, &mut payload).await?;
        req.set_payload(Payload::from(bytes.clone()));

        let problem = |message: String| Problem {
            location: "body",
            name: "body",
            pointer: String::new(),
            message,
        };
        if bytes.is_empty() {
            let problems = if self.required {
                vec![problem("is required".to_string())]
            } else {
                Vec::new()
            };
            return Ok(BodyOutcome::Checked(problems));
        }
        if !is_json(req.headers().get(CONTENT_TYPE)) {
            return Ok(BodyOutcome::UnsupportedMediaType);
        }
        let value: Value = match serde_json::from_slice(&bytes) {
            Ok(value) => value,
            Err(err) => {
                return Ok(BodyOutcome::Checked(vec![problem(format!(
                    "invalid JSON: {err}"
                ))]));
            }
        };
        let problems = self
            .validator
            .iter_errors(&value)
            .map(|error| Problem {
                pointer: error.instance_path().to_string(),
                message: error.to_string(),
                ..problem(String::new())
            })
            .collect();
        Ok(BodyOutcome::Checked(problems))
    }
}

impl Operation {
    fn check_parameters(&self, req: &HttpRequest) -> Vec<Problem> {
        let operation = self;
        let query = serde_urlencoded::from_str::<Vec<(String, String)>>(req.query_string());
        let mut problems = Vec::new();
        if query.is_err() {
            problems.push(Problem {
                location: Location::Query.as_str(),
                name: "",
                pointer: String::new(),
                message: "malformed query string".to_string(),
            });
        }
        let query = query.unwrap_or_default();
        for parameter in &operation.parameters {
            let Some(value) = parameter_value(req, &query, parameter) else {
                if parameter.spec.required {
                    problems.push(Problem {
                        location: parameter.spec.location.as_str(),
                        name: parameter.spec.name,
                        pointer: String::new(),
                        message: "is required".to_string(),
                    });
                }
                continue;
            };
            problems.extend(
                parameter
                    .validator
                    .iter_errors(&value)
                    .map(|error| Problem {
                        location: parameter.spec.location.as_str(),
                        name: parameter.spec.name,
                        pointer: error.instance_path().to_string(),
                        message: error.to_string(),
                    }),
            );
        }
        problems
    }
}

/// The value of a parameter, `None` when the request does not carry it.
fn parameter_value(
    req: &HttpRequest,
    query: &[(String, String)],
    parameter: &Parameter,
) -> Option<Value> {
    let document = &parameter.schema;
    let schema = resolve(document, document);
    if primary_type(schema) == Some("object") {
        return Some(match object_entries(req, query, &parameter.spec, schema)? {
            Ok(entries) => object_value(&entries, schema, document),
            Err(raw) => Value::from(raw),
        });
    }
    let raw = raw_values(req, query, &parameter.spec);
    (!raw.is_empty()).then(|| coerce(&raw, &parameter.spec, document))
}

/// The raw values a parameter has in the request. Empty when it is absent.
fn raw_values(req: &HttpRequest, query: &[(String, String)], spec: &ParameterSpec) -> Vec<String> {
    match spec.location {
        Location::Path => req
            .match_info()
            .get(spec.name)
            .map(str::to_string)
            .into_iter()
            .collect(),
        Location::Query => query
            .iter()
            .filter(|(name, _)| name == spec.name)
            .map(|(_, value)| value.clone())
            .collect(),
        Location::Header => req
            .headers()
            .get_all(spec.name)
            .filter_map(|value| value.to_str().ok())
            .map(str::to_string)
            .collect(),
        Location::Cookie => req
            .cookie(spec.name)
            .map(|cookie| cookie.value().to_string())
            .into_iter()
            .collect(),
    }
}

/// Follow the `$ref` of `schema` to the schema actually describing the value.
fn resolve<'a>(schema: &'a Value, document: &'a Value) -> &'a Value {
    let mut schema = schema;
    for _ in 0..MAX_REF_DEPTH {
        let Some(reference) = schema.get("$ref").and_then(Value::as_str) else {
            break;
        };
        let Some(target) = reference
            .strip_prefix(SCHEMA_REF_PREFIX)
            .and_then(|name| document.pointer(&format!("/components/schemas/{name}")))
        else {
            break;
        };
        schema = target;
    }
    schema
}

/// The type of a schema, ignoring `null`.
fn primary_type(schema: &Value) -> Option<&str> {
    match schema.get("type")? {
        Value::String(schema_type) => Some(schema_type),
        Value::Array(types) => types
            .iter()
            .filter_map(Value::as_str)
            .find(|schema_type| *schema_type != "null"),
        _ => None,
    }
}

/// Parameters are transported as strings: turn them into the JSON value the
/// schema is about. A value that does not fit is left as a string, which the
/// validation then reports.
fn coerce_scalar(raw: &str, schema: &Value, document: &Value) -> Value {
    match primary_type(resolve(schema, document)) {
        Some("integer") => raw
            .parse::<i64>()
            .map(Value::from)
            .or_else(|_| raw.parse::<u64>().map(Value::from))
            .unwrap_or_else(|_| Value::from(raw)),
        Some("number") => raw
            .parse::<i64>()
            .map(Value::from)
            .ok()
            .or_else(|| {
                raw.parse::<f64>()
                    .ok()
                    .and_then(Number::from_f64)
                    .map(Value::Number)
            })
            .unwrap_or_else(|| Value::from(raw)),
        Some("boolean") => match raw {
            "true" => Value::Bool(true),
            "false" => Value::Bool(false),
            _ => Value::from(raw),
        },
        _ => Value::from(raw),
    }
}

fn style(spec: &ParameterSpec) -> &'static str {
    spec.style.unwrap_or(match spec.location {
        Location::Query | Location::Cookie => "form",
        Location::Path | Location::Header => "simple",
    })
}

/// The separator of the items of an array parameter, `None` when every item
/// is a distinct occurrence of the parameter (`form` style, exploded).
fn item_separator(spec: &ParameterSpec) -> Option<char> {
    match style(spec) {
        "form" if spec.explode.unwrap_or(true) => None,
        "spaceDelimited" => Some(' '),
        "pipeDelimited" => Some('|'),
        _ => Some(','),
    }
}

/// The properties of an object parameter, as `(name, raw value)` pairs.
///
/// `None` when the request does not carry the parameter, `Err` with the raw
/// value when it is not a well formed object.
fn object_entries(
    req: &HttpRequest,
    query: &[(String, String)],
    spec: &ParameterSpec,
    schema: &Value,
) -> Option<Result<Vec<(String, String)>, String>> {
    let style = style(spec);
    let explode = spec.explode.unwrap_or(style == "form");
    if spec.location == Location::Query && style == "deepObject" {
        // ?name[property]=value
        let entries: Vec<_> = query
            .iter()
            .filter_map(|(key, value)| {
                let property = key
                    .strip_prefix(spec.name)?
                    .strip_prefix('[')?
                    .strip_suffix(']')?;
                Some((property.to_string(), value.clone()))
            })
            .collect();
        return (!entries.is_empty()).then_some(Ok(entries));
    }
    if spec.location == Location::Query && style == "form" && explode {
        // ?property=value: the name of the parameter is not part of the
        // request, only the declared properties tell what belongs to it.
        let declared = schema.get("properties").and_then(Value::as_object);
        let entries: Vec<_> = query
            .iter()
            .filter(|(key, _)| declared.is_some_and(|declared| declared.contains_key(key)))
            .cloned()
            .collect();
        return (!entries.is_empty()).then_some(Ok(entries));
    }

    let raw = raw_values(req, query, spec).into_iter().next()?;
    if raw.is_empty() {
        return Some(Ok(Vec::new()));
    }
    let parts: Vec<&str> = raw.split(item_separator(spec).unwrap_or(',')).collect();
    let entries = if explode {
        // property=value,property=value
        parts
            .iter()
            .map(|part| {
                part.split_once('=')
                    .map(|(k, v)| (k.to_string(), v.to_string()))
            })
            .collect::<Option<Vec<_>>>()
    } else if parts.len().is_multiple_of(2) {
        // property,value,property,value
        Some(
            parts
                .chunks(2)
                .map(|pair| (pair[0].to_string(), pair[1].to_string()))
                .collect(),
        )
    } else {
        None
    };
    Some(entries.ok_or(raw))
}

/// Build the object from its properties, each one coerced to its own type.
/// The properties the schema does not describe are left as strings.
fn object_value(entries: &[(String, String)], schema: &Value, document: &Value) -> Value {
    let properties = schema.get("properties");
    let additional = schema.get("additionalProperties");
    Value::Object(
        entries
            .iter()
            .map(|(name, raw)| {
                let property_schema = properties
                    .and_then(|properties| properties.get(name))
                    .or(additional)
                    .unwrap_or(&Value::Null);
                (name.clone(), coerce_scalar(raw, property_schema, document))
            })
            .collect(),
    )
}

fn coerce(raw: &[String], spec: &ParameterSpec, schema: &Value) -> Value {
    if primary_type(resolve(schema, schema)) == Some("array") {
        let items = resolve(schema, schema).get("items").unwrap_or(&Value::Null);
        let pieces: Vec<&str> = match item_separator(spec) {
            Some(separator) => raw
                .iter()
                .flat_map(|value| value.split(separator))
                .collect(),
            None => raw.iter().map(String::as_str).collect(),
        };
        Value::Array(
            pieces
                .into_iter()
                .map(|piece| coerce_scalar(piece, items, schema))
                .collect(),
        )
    } else {
        coerce_scalar(&raw[0], schema, schema)
    }
}

type ResponseFuture<B> =
    Pin<Box<dyn Future<Output = Result<ServiceResponse<EitherBody<B>>, Error>>>>;

impl<S, B> Transform<S, ServiceRequest> for Validation
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Transform = ValidationMiddleware<S>;
    type InitError = ();
    type Future = Ready<Result<Self::Transform, Self::InitError>>;

    fn new_transform(&self, service: S) -> Self::Future {
        ready(Ok(ValidationMiddleware {
            service: Rc::new(service),
            validation: self.clone(),
        }))
    }
}

pub struct ValidationMiddleware<S> {
    service: Rc<S>,
    validation: Validation,
}

fn errors_response(
    mut response: actix_web::HttpResponseBuilder,
    problems: Vec<Problem>,
) -> HttpResponse {
    let errors: Vec<Value> = problems
        .into_iter()
        .map(|problem| {
            json!({
                "in": problem.location,
                "name": problem.name,
                "pointer": problem.pointer,
                "message": problem.message,
            })
        })
        .collect();
    response.json(json!({ "errors": errors }))
}

impl<S, B> Service<ServiceRequest> for ValidationMiddleware<S>
where
    S: Service<ServiceRequest, Response = ServiceResponse<B>, Error = Error> + 'static,
    B: 'static,
{
    type Response = ServiceResponse<EitherBody<B>>;
    type Error = Error;
    type Future = ResponseFuture<B>;

    forward_ready!(service);

    fn call(&self, mut req: ServiceRequest) -> Self::Future {
        let service = Rc::clone(&self.service);
        let validation = self.validation.clone();
        Box::pin(async move {
            let mut problems = Vec::new();
            if let Some(operation) = validation.operation(req.method().as_str()) {
                problems = operation.check_parameters(req.request());
                if let Some(body) = &operation.body {
                    match body.check(&mut req).await? {
                        BodyOutcome::Checked(body_problems) => problems.extend(body_problems),
                        BodyOutcome::UnsupportedMediaType => {
                            let problem = Problem {
                                location: Location::Header.as_str(),
                                name: "Content-Type",
                                pointer: String::new(),
                                message: "expected application/json".to_string(),
                            };
                            let response = errors_response(
                                HttpResponse::UnsupportedMediaType(),
                                vec![problem],
                            );
                            return Ok(req.into_response(response).map_into_right_body());
                        }
                    }
                }
            }
            if problems.is_empty() {
                return service
                    .call(req)
                    .await
                    .map(ServiceResponse::map_into_left_body);
            }
            let response = errors_response(HttpResponse::BadRequest(), problems);
            Ok(req.into_response(response).map_into_right_body())
        })
    }
}
