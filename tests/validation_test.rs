use actix_web::{
    App, HttpResponse,
    http::{self, StatusCode},
    test, web,
};
use serde_json::{Value, json};

use autoroute::gen_config_from;

async fn ok_handler() -> HttpResponse {
    HttpResponse::Ok().finish()
}

gen_config_from!(
    r##"
openapi: 3.0.0
info:
  title: Validation API
  version: 0.0.1
paths:
  /items/{itemId}:
    x-autoroute-resource: "item"
    parameters:
      - name: itemId
        in: path
        schema: {type: integer, minimum: 1}
    get:
      x-autoroute-validate: true
      x-autoroute-handler: ok_handler
      parameters:
        - name: limit
          in: query
          schema: {type: integer, minimum: 1, maximum: 100}
        - name: kind
          in: query
          required: true
          schema: {type: string, enum: [a, b]}
        - name: tags
          in: query
          schema: {type: array, items: {type: string}, maxItems: 2}
        - name: ids
          in: query
          explode: false
          schema: {type: array, items: {type: integer}}
        - name: X-Verbose
          in: header
          schema: {type: boolean}
        - $ref: "#/components/parameters/Ratio"
        - name: page
          in: query
          schema:
            type: object
            required: [offset]
            properties:
              offset: {type: integer, minimum: 0}
              size: {type: integer, maximum: 50}
        - name: filter
          in: query
          style: deepObject
          schema:
            type: object
            properties:
              min: {type: integer}
              max: {type: integer}
        - name: sort
          in: query
          explode: false
          schema:
            type: object
            properties:
              by: {type: string}
              desc: {type: boolean}
        - name: X-Range
          in: header
          explode: true
          schema:
            type: object
            properties:
              from: {type: integer}
              to: {type: integer}
      responses:
        '200':
          description: success
    delete:
      x-autoroute-handler: ok_handler
      responses:
        '200':
          description: success
components:
  parameters:
    Ratio:
      name: ratio
      in: query
      schema: {$ref: "#/components/schemas/Ratio"}
  schemas:
    Ratio:
      type: number
      maximum: 1
      exclusiveMaximum: true
"##
);

async fn call(
    uri: &str,
    method: http::Method,
    header: Option<(&str, &str)>,
) -> (StatusCode, Value) {
    let service =
        test::init_service(App::new().service(web::scope("/api").configure(autoroute_config)))
            .await;
    let mut req = test::TestRequest::with_uri(uri).method(method);
    if let Some((name, value)) = header {
        req = req.insert_header((name, value));
    }
    let resp = test::call_service(&service, req.to_request()).await;
    let status = resp.status();
    let body = test::read_body(resp).await;
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

async fn get(uri: &str) -> (StatusCode, Value) {
    call(uri, http::Method::GET, None).await
}

fn error_locations(body: &Value) -> Vec<(String, String)> {
    body["errors"]
        .as_array()
        .expect("errors array")
        .iter()
        .map(|e| {
            (
                e["in"].as_str().unwrap().to_string(),
                e["name"].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[actix_rt::test]
async fn test_valid_request_reaches_the_handler() {
    let (status, _) = get("/api/items/3?kind=a&limit=10&tags=x&tags=y&ids=1,2,3&ratio=0.5").await;
    assert_eq!(status, StatusCode::OK);
}

#[actix_rt::test]
async fn test_required_parameter_missing() {
    let (status, body) = get("/api/items/3").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        error_locations(&body),
        [("query".to_string(), "kind".to_string())]
    );
    assert_eq!(body["errors"][0]["message"], json!("is required"));
}

#[actix_rt::test]
async fn test_wrong_type_and_bounds_are_reported_together() {
    let (status, body) = get("/api/items/abc?kind=c&limit=1000&ratio=1").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let mut locations = error_locations(&body);
    locations.sort();
    assert_eq!(
        locations,
        [
            ("path".to_string(), "itemId".to_string()),
            ("query".to_string(), "kind".to_string()),
            ("query".to_string(), "limit".to_string()),
            ("query".to_string(), "ratio".to_string()),
        ]
    );
}

#[actix_rt::test]
async fn test_not_a_number() {
    let (status, _) = get("/api/items/3?kind=a&limit=ten").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[actix_rt::test]
async fn test_array_parameters() {
    let (status, _) = get("/api/items/3?kind=a&tags=x&tags=y&tags=z").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "maxItems");
    let (status, _) = get("/api/items/3?kind=a&ids=1,two").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "item type");
}

#[actix_rt::test]
async fn test_header_parameter() {
    let get = http::Method::GET;
    let (status, _) = call(
        "/api/items/3?kind=a",
        get.clone(),
        Some(("X-Verbose", "true")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = call("/api/items/3?kind=a", get, Some(("X-Verbose", "maybe"))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        error_locations(&body),
        [("header".to_string(), "X-Verbose".to_string())]
    );
}

#[actix_rt::test]
async fn test_operation_without_validation_is_untouched() {
    let (status, _) = call("/api/items/abc", http::Method::DELETE, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[actix_rt::test]
async fn test_form_exploded_object_parameter() {
    let (status, _) = get("/api/items/3?kind=a&offset=10&size=5").await;
    assert_eq!(status, StatusCode::OK);

    let (status, body) = get("/api/items/3?kind=a&offset=abc").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        error_locations(&body),
        [("query".to_string(), "page".to_string())]
    );
    assert_eq!(body["errors"][0]["pointer"], json!("/offset"));

    let (status, _) = get("/api/items/3?kind=a&offset=1&size=51").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "size above maximum");
}

#[actix_rt::test]
async fn test_required_property_missing_from_object_parameter() {
    // `size` alone makes the `page` object present, but `offset` is required.
    let (status, _) = get("/api/items/3?kind=a&size=5").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[actix_rt::test]
async fn test_deep_object_parameter() {
    let (status, _) = get("/api/items/3?kind=a&filter%5Bmin%5D=1&filter%5Bmax%5D=9").await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = get("/api/items/3?kind=a&filter%5Bmin%5D=1&filter%5Bmax%5D=nine").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["errors"][0]["pointer"], json!("/max"));
}

#[actix_rt::test]
async fn test_form_object_parameter_not_exploded() {
    let (status, _) = get("/api/items/3?kind=a&sort=by,name,desc,true").await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = get("/api/items/3?kind=a&sort=by,name,desc,maybe").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "property type");
    let (status, _) = get("/api/items/3?kind=a&sort=by,name,desc").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "odd number of items");
}

#[actix_rt::test]
async fn test_simple_exploded_object_header() {
    let get = http::Method::GET;
    let (status, _) = call(
        "/api/items/3?kind=a",
        get.clone(),
        Some(("X-Range", "from=1,to=5")),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = call("/api/items/3?kind=a", get, Some(("X-Range", "from=1,to=x"))).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}
