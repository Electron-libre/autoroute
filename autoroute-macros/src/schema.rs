//! Conversion of `OpenAPI` 3.0 Schema Objects to standalone JSON Schema documents.
//!
//! The `OpenAPI` 3.0 Schema Object is an extended subset of JSON Schema draft 4:
//! it adds a few keywords (`nullable`, `example`, ...) that validators do not
//! know about, and refers to reusable schemas through `#/components/schemas/*`.
//! The documents produced here embed the schemas they reference, so that they
//! can be compiled by a plain JSON Schema (draft 4) validator.

use std::collections::BTreeSet;

use proc_macro_error::abort_call_site;
use serde_json::{Map, Value};

const SCHEMA_REF_PREFIX: &str = "#/components/schemas/";

/// Keywords that only carry documentation and are not part of JSON Schema.
const OPENAPI_ONLY_KEYWORDS: [&str; 5] = [
    "example",
    "discriminator",
    "xml",
    "externalDocs",
    "deprecated",
];

/// Follow a local `$ref` to an entry of `components/<kind>`.
///
/// Aborts the compilation when the reference is not local or dangling.
pub(crate) fn resolve_component<'a>(root: &'a Value, kind: &str, reference: &str) -> &'a Value {
    let prefix = format!("#/components/{kind}/");
    let Some(name) = reference.strip_prefix(&prefix) else {
        abort_call_site!(
            "Unsupported reference {:?}: only local references to `{}*` are supported",
            reference,
            prefix
        );
    };
    let Some(component) = root
        .get("components")
        .and_then(|components| components.get(kind))
        .and_then(|entries| entries.get(name))
    else {
        abort_call_site!("Unresolved reference {:?}", reference);
    };
    component
}

/// Convert an `OpenAPI` Schema Object to a JSON Schema, collecting the
/// `$ref` it uses in `refs`.
fn convert(schema: &Value, refs: &mut Vec<String>) -> Value {
    let Value::Object(schema) = schema else {
        return schema.clone();
    };
    let mut converted = Map::new();
    for (keyword, value) in schema {
        let value = match keyword.as_str() {
            keyword if OPENAPI_ONLY_KEYWORDS.contains(&keyword) => continue,
            "nullable" => continue,
            "$ref" => {
                if let Value::String(reference) = value {
                    refs.push(reference.clone());
                }
                value.clone()
            }
            "properties" => match value {
                Value::Object(properties) => Value::Object(
                    properties
                        .iter()
                        .map(|(name, schema)| (name.clone(), convert(schema, refs)))
                        .collect(),
                ),
                _ => value.clone(),
            },
            "items" | "not" | "additionalProperties" => convert(value, refs),
            "allOf" | "oneOf" | "anyOf" => match value {
                Value::Array(schemas) => {
                    Value::Array(schemas.iter().map(|s| convert(s, refs)).collect())
                }
                _ => value.clone(),
            },
            _ => value.clone(),
        };
        converted.insert(keyword.clone(), value);
    }
    if schema.get("nullable") == Some(&Value::Bool(true)) {
        allow_null(&mut converted);
    }
    Value::Object(converted)
}

/// Translate `nullable: true` into a `type` that accepts `null`.
fn allow_null(schema: &mut Map<String, Value>) {
    match schema.get_mut("type") {
        Some(schema_type @ Value::String(_)) => {
            *schema_type = Value::Array(vec![schema_type.clone(), Value::from("null")]);
        }
        Some(Value::Array(types)) if !types.contains(&Value::from("null")) => {
            types.push(Value::from("null"));
        }
        _ => {}
    }
}

/// Build a standalone JSON Schema from an `OpenAPI` Schema Object.
///
/// The `components/schemas` reachable from `schema` are embedded under the
/// same `#/components/schemas` path, so that the `$ref` keep resolving.
pub(crate) fn standalone(root: &Value, schema: &Value) -> Value {
    let mut refs = Vec::new();
    let mut document = convert(schema, &mut refs);

    let mut embedded = Map::new();
    let mut visited = BTreeSet::new();
    while let Some(reference) = refs.pop() {
        if !visited.insert(reference.clone()) {
            continue;
        }
        let component = resolve_component(root, "schemas", &reference);
        let name = &reference[SCHEMA_REF_PREFIX.len()..];
        embedded.insert(name.to_string(), convert(component, &mut refs));
    }

    if let Value::Object(document) = &mut document
        && !embedded.is_empty()
    {
        let mut components = Map::new();
        components.insert("schemas".to_string(), Value::Object(embedded));
        document.insert("components".to_string(), Value::Object(components));
    }
    document
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn test_nullable_becomes_type_union() {
        let schema = json!({"type": "string", "nullable": true, "example": "a"});
        let document = standalone(&json!({}), &schema);
        assert_eq!(document, json!({"type": ["string", "null"]}));
    }

    #[test]
    fn test_conversion_recurses_in_schema_positions_only() {
        let schema = json!({
            "type": "object",
            "properties": {
                "example": {"type": "integer", "nullable": true},
                "tags": {"type": "array", "items": {"type": "string", "nullable": true}}
            },
            "default": {"nullable": true}
        });
        let document = standalone(&json!({}), &schema);
        assert_eq!(
            document,
            json!({
                "type": "object",
                "properties": {
                    "example": {"type": ["integer", "null"]},
                    "tags": {"type": "array", "items": {"type": ["string", "null"]}}
                },
                "default": {"nullable": true}
            })
        );
    }

    #[test]
    fn test_referenced_schemas_are_embedded_transitively() {
        let root = json!({"components": {"schemas": {
            "Foo": {"type": "object", "properties": {"bar": {"$ref": "#/components/schemas/Bar"}}},
            "Bar": {"type": "string", "nullable": true},
            "Unused": {"type": "integer"}
        }}});
        let document = standalone(&root, &json!({"$ref": "#/components/schemas/Foo"}));
        assert_eq!(
            document,
            json!({
                "$ref": "#/components/schemas/Foo",
                "components": {"schemas": {
                    "Foo": {"type": "object", "properties": {"bar": {"$ref": "#/components/schemas/Bar"}}},
                    "Bar": {"type": ["string", "null"]}
                }}
            })
        );
    }

    #[test]
    fn test_recursive_schema_terminates() {
        let root = json!({"components": {"schemas": {
            "Node": {"type": "object", "properties": {"next": {"$ref": "#/components/schemas/Node"}}}
        }}});
        let document = standalone(&root, &json!({"$ref": "#/components/schemas/Node"}));
        assert!(document.pointer("/components/schemas/Node").is_some());
    }
}
