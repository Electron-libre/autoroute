use actix_web::{
    App, HttpResponse,
    http::{self, StatusCode},
    test, web,
};
use serde_json::{Value, json};

use autoroute::gen_config_from;

/// Sends back the payload it received: it proves the handler still gets
/// the body the validation middleware has already read.
async fn echo_handler(body: web::Bytes) -> HttpResponse {
    HttpResponse::Ok().body(body)
}

gen_config_from!(
    r##"
openapi: 3.0.0
info:
  title: Body validation API
  version: 0.0.1
x-autoroute-validate: true
paths:
  /things/{thingId}:
    parameters:
      - name: thingId
        in: path
        schema: {type: integer}
    post:
      x-autoroute-handler: echo_handler
      requestBody:
        required: true
        content:
          application/json:
            schema: {$ref: "#/components/schemas/Thing"}
      responses:
        '200':
          description: success
    put:
      x-autoroute-handler: echo_handler
      requestBody:
        $ref: "#/components/requestBodies/OptionalTags"
      responses:
        '200':
          description: success
    delete:
      x-autoroute-handler: echo_handler
      responses:
        '200':
          description: success
components:
  requestBodies:
    OptionalTags:
      content:
        application/json:
          schema:
            type: array
            items: {type: string}
  schemas:
    Thing:
      type: object
      required: [name]
      additionalProperties: false
      properties:
        name: {type: string, minLength: 2}
        size: {type: integer, minimum: 0}
        owner: {$ref: "#/components/schemas/Owner"}
    Owner:
      type: object
      nullable: true
      properties:
        id: {type: integer}
"##
);

const THING_URI: &str = "/api/things/1";

async fn send(
    request: test::TestRequest,
    payload_limit: Option<usize>,
) -> (StatusCode, web::Bytes) {
    let mut app = App::new();
    if let Some(limit) = payload_limit {
        app = app.app_data(web::PayloadConfig::new(limit));
    }
    let service =
        test::init_service(app.service(web::scope("/api").configure(autoroute_config))).await;
    // Errors of the service, like an overflowing payload, are turned into
    // responses by the http server.
    match test::try_call_service(&service, request.to_request()).await {
        Ok(resp) => {
            let status = resp.status();
            (status, test::read_body(resp).await)
        }
        Err(err) => (err.error_response().status(), web::Bytes::new()),
    }
}

fn json_request(method: http::Method, uri: &str, body: &str) -> test::TestRequest {
    test::TestRequest::with_uri(uri)
        .method(method)
        .insert_header((http::header::CONTENT_TYPE, "application/json"))
        .set_payload(body.to_string())
}

async fn post(body: &str) -> (StatusCode, Value) {
    let (status, body) = send(json_request(http::Method::POST, THING_URI, body), None).await;
    (status, serde_json::from_slice(&body).unwrap_or(Value::Null))
}

#[actix_rt::test]
async fn test_valid_body_reaches_the_handler_intact() {
    let payload = r#"{"name":"widget","size":3,"owner":null}"#;
    let (status, body) = send(json_request(http::Method::POST, THING_URI, payload), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, payload);
}

#[actix_rt::test]
async fn test_body_violations_are_reported_with_their_pointer() {
    let (status, body) = post(r#"{"name":"w","size":-1,"owner":{"id":"x"}}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let mut pointers: Vec<_> = body["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            assert_eq!(e["in"], json!("body"));
            e["pointer"].as_str().unwrap().to_string()
        })
        .collect();
    pointers.sort();
    assert_eq!(pointers, ["/name", "/owner/id", "/size"]);
}

#[actix_rt::test]
async fn test_missing_required_property_and_unknown_property() {
    let (status, _) = post(r#"{"size":1}"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "name is required");
    let (status, _) = post(r#"{"name":"widget","color":"red"}"#).await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "additionalProperties: false"
    );
}

#[actix_rt::test]
async fn test_malformed_json() {
    let (status, body) = post(r#"{"name":"#).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert!(
        body["errors"][0]["message"]
            .as_str()
            .unwrap()
            .starts_with("invalid JSON")
    );
}

#[actix_rt::test]
async fn test_required_body_missing() {
    let request = test::TestRequest::post().uri(THING_URI);
    let (status, body) = send(request, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["errors"][0]["message"], json!("is required"));
}

#[actix_rt::test]
async fn test_optional_body_may_be_omitted() {
    let (status, _) = send(test::TestRequest::put().uri(THING_URI), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        json_request(http::Method::PUT, THING_URI, r#"["a","b"]"#),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = send(
        json_request(http::Method::PUT, THING_URI, r#"["a",1]"#),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
}

#[actix_rt::test]
async fn test_unsupported_media_type() {
    let request = test::TestRequest::post()
        .uri(THING_URI)
        .insert_header((http::header::CONTENT_TYPE, "text/plain"))
        .set_payload(r#"{"name":"widget"}"#);
    let (status, body) = send(request, None).await;
    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    let body: Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(body["errors"][0]["name"], json!("Content-Type"));

    let request = test::TestRequest::post()
        .uri(THING_URI)
        .set_payload(r#"{"name":"widget"}"#);
    let (status, _) = send(request, None).await;
    assert_eq!(
        status,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "no content type"
    );
}

#[actix_rt::test]
async fn test_json_media_type_variants() {
    for content_type in [
        "application/json; charset=utf-8",
        "application/vnd.api+json",
    ] {
        let request = test::TestRequest::post()
            .uri(THING_URI)
            .insert_header((http::header::CONTENT_TYPE, content_type))
            .set_payload(r#"{"name":"widget"}"#);
        let (status, _) = send(request, None).await;
        assert_eq!(status, StatusCode::OK, "{content_type}");
    }
}

#[actix_rt::test]
async fn test_payload_limit_is_enforced() {
    let request = json_request(http::Method::POST, THING_URI, r#"{"name":"widget"}"#);
    let (status, _) = send(request, Some(4)).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

#[actix_rt::test]
async fn test_parameter_and_body_problems_are_reported_together() {
    let request = json_request(http::Method::POST, "/api/things/abc", r#"{"size":1}"#);
    let (status, body) = send(request, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let body: Value = serde_json::from_slice(&body).unwrap();
    let mut sources: Vec<_> = body["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| e["in"].as_str().unwrap())
        .collect();
    sources.sort_unstable();
    assert_eq!(sources, ["body", "path"]);
}

#[actix_rt::test]
async fn test_operation_without_body_spec_is_untouched() {
    let (status, _) = send(test::TestRequest::delete().uri(THING_URI), None).await;
    assert_eq!(status, StatusCode::OK);
}
