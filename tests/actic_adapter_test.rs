use actix_web::{http, test, web, App, HttpRequest, HttpResponse};

use actix_web::http::Method;
use autoroute::gen_config_from_path;

fn test_handler(req: HttpRequest) -> HttpResponse {
    HttpResponse::Ok().body(format! {"{:?}", req})
}

const TEST_SCOPE: &str = "/api";
const TEST_URI: &str = "/api/foo/1";

#[actix_rt::test]
async fn test_get() {
    gen_config_from_path!("tests/test_api.yml");
    let mut test_service =
        test::init_service(App::new().service(web::scope(TEST_SCOPE).configure(autoroute_config)))
            .await;
    let req = test::TestRequest::get().uri(TEST_URI).to_request();

    let resp = test::call_service(&mut test_service, req).await;
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[actix_rt::test]
async fn test_post() {
    gen_config_from_path!("tests/test_api.yml");

    let mut test_service =
        test::init_service(App::new().service(web::scope(TEST_SCOPE).configure(autoroute_config)))
            .await;
    let req = test::TestRequest::post().uri(TEST_URI).to_request();

    let resp = test::call_service(&mut test_service, req).await;
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[actix_rt::test]
async fn test_put() {
    gen_config_from_path!("tests/test_api.yml");

    let mut test_service =
        test::init_service(App::new().service(web::scope(TEST_SCOPE).configure(autoroute_config)))
            .await;
    let req = test::TestRequest::put().uri(TEST_URI).to_request();

    let resp = test::call_service(&mut test_service, req).await;
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[actix_rt::test]
async fn test_delete() {
    gen_config_from_path!("tests/test_api.yml");

    let mut test_service =
        test::init_service(App::new().service(web::scope(TEST_SCOPE).configure(autoroute_config)))
            .await;
    let req = test::TestRequest::delete().uri(TEST_URI).to_request();

    let resp = test::call_service(&mut test_service, req).await;
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[actix_rt::test]
async fn test_patch() {
    gen_config_from_path!("tests/test_api.yml");

    let mut test_service =
        test::init_service(App::new().service(web::scope(TEST_SCOPE).configure(autoroute_config)))
            .await;
    let req = test::TestRequest::patch().uri(TEST_URI).to_request();

    let resp = test::call_service(&mut test_service, req).await;
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[actix_rt::test]
async fn test_head() {
    gen_config_from_path!("tests/test_api.yml");

    let mut test_service =
        test::init_service(App::new().service(web::scope(TEST_SCOPE).configure(autoroute_config)))
            .await;
    let req = test::TestRequest::with_uri(TEST_URI)
        .method(Method::HEAD)
        .to_request();

    let resp = test::call_service(&mut test_service, req).await;
    assert_eq!(resp.status(), http::StatusCode::OK);
}
