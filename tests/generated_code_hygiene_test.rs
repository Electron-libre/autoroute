//! The generated code must not depend on what the caller imported.

async fn ok_handler() -> actix_web::HttpResponse {
    actix_web::HttpResponse::Ok().finish()
}

mod generated {
    use super::ok_handler;

    autoroute::gen_config_from!(
        r##"
openapi: 3.0.0
info:
  title: Hygiene API
  version: 0.0.1
paths:
  /foo/{fooId}:
    x-autoroute-resource: "foo"
    get:
      x-autoroute-handler: ok_handler
      x-autoroute-validate: true
      parameters:
        - name: fooId
          in: path
          schema: {type: integer}
      requestBody:
        content:
          application/json:
            schema: {type: object}
"##
    );

    #[actix_rt::test]
    async fn test_generated_code_is_self_contained() {
        let service = actix_web::test::init_service(
            actix_web::App::new()
                .service(actix_web::web::scope("/api").configure(autoroute_config)),
        )
        .await;
        let req = actix_web::test::TestRequest::with_uri("/api/foo/1").to_request();
        let resp = actix_web::test::call_service(&service, req).await;
        assert_eq!(resp.status(), actix_web::http::StatusCode::OK);
    }
}
