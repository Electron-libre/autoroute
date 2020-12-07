use actix_web::{
    http::{self, Method},
    test, web, App, HttpRequest, HttpResponse,
};
use serde::{Deserialize, Serialize};

use autoroute::gen_config_from_path;

#[derive(Deserialize, Serialize)]
struct TestHandlerResponse {
    path_param: String,
    reflected_url: String,
}

fn test_handler(req: HttpRequest) -> HttpResponse {
    let path_param = req
        .match_info()
        .get("fooId")
        .expect("param fooId is missing")
        .to_string();
    let reflected_url = req.url_for("foo", &["1"]).unwrap().to_string();

    let resp = TestHandlerResponse {
        path_param,
        reflected_url,
    };
    HttpResponse::Ok().json(resp)
}

const TEST_SCOPE: &str = "/api";
const TEST_URI: &str = "/api/foo/1";

#[actix_rt::test]
async fn test_supported_methods() {
    gen_config_from_path!("tests/test_api.yml");
    let mut test_service =
        test::init_service(App::new().service(web::scope(TEST_SCOPE).configure(autoroute_config)))
            .await;

    let methods = vec![
        "GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS", "TRACE",
    ];
    for method in methods {
        let http_method = Method::from_bytes(method.as_bytes()).unwrap();

        let req = test::TestRequest::with_uri(TEST_URI)
            .method(http_method)
            .to_request();

        let resp = test::call_service(&mut test_service, req).await;
        assert_eq!(
            resp.status(),
            http::StatusCode::OK,
            "Failed for method {}",
            method
        );
    }
}

#[actix_rt::test]
async fn test_path_params() {
    gen_config_from_path!("tests/test_api.yml");
    let mut test_service =
        test::init_service(App::new().service(web::scope(TEST_SCOPE).configure(autoroute_config)))
            .await;

    let req = test::TestRequest::with_uri(TEST_URI)
        .method(http::Method::GET)
        .to_request();

    let resp = test::call_service(&mut test_service, req).await;
    let json_resp: TestHandlerResponse = test::read_body_json(resp).await;
    assert_eq!(json_resp.path_param, "1",);
}

#[actix_rt::test]
async fn test_url_reflection() {
    gen_config_from_path!("tests/test_api.yml");
    let mut test_service =
        test::init_service(App::new().service(web::scope(TEST_SCOPE).configure(autoroute_config)))
            .await;

    let req = test::TestRequest::with_uri(TEST_URI)
        .method(http::Method::GET)
        .to_request();

    let resp = test::call_service(&mut test_service, req).await;
    let json_resp: TestHandlerResponse = test::read_body_json(resp).await;
    assert_eq!(json_resp.reflected_url, "http://localhost:8080/api/foo/1",);
}
