use actix_web::{http, test, web, App, HttpRequest, HttpResponse};

use actix_web::http::Method;
use autoroute::gen_config_from_path;

fn return_request_as_response(req: HttpRequest) -> HttpResponse {
    HttpResponse::Ok().body(format! {"{:?}", req})
}

fn test_handler_get(req: HttpRequest) -> HttpResponse {
    return_request_as_response(req)
}

fn test_handler_post(req: HttpRequest) -> HttpResponse {
    return_request_as_response(req)
}

fn test_handler_put(req: HttpRequest) -> HttpResponse {
    return_request_as_response(req)
}

fn test_handler_delete(req: HttpRequest) -> HttpResponse {
    return_request_as_response(req)
}

fn test_handler_patch(req: HttpRequest) -> HttpResponse {
    return_request_as_response(req)
}

fn test_handler_head(req: HttpRequest) -> HttpResponse {
    return_request_as_response(req)
}

fn test_handler_options(req: HttpRequest) -> HttpResponse {
    return_request_as_response(req)
}

#[actix_rt::test]
async fn test_get() {
    gen_config_from_path!("tests/test_api.yml");
    let mut test_service =
        test::init_service(App::new().service(web::scope("/api").configure(autoroute_config)))
            .await;
    let req = test::TestRequest::get().uri("/api/foo").to_request();

    let resp = test::call_service(&mut test_service, req).await;
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[actix_rt::test]
async fn test_post() {
    gen_config_from_path!("tests/test_api.yml");

    let mut test_service =
        test::init_service(App::new().service(web::scope("/api").configure(autoroute_config)))
            .await;
    let req = test::TestRequest::post().uri("/api/foo").to_request();

    let resp = test::call_service(&mut test_service, req).await;
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[actix_rt::test]
async fn test_put() {
    gen_config_from_path!("tests/test_api.yml");

    let mut test_service =
        test::init_service(App::new().service(web::scope("/api").configure(autoroute_config)))
            .await;
    let req = test::TestRequest::put().uri("/api/foo").to_request();

    let resp = test::call_service(&mut test_service, req).await;
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[actix_rt::test]
async fn test_delete() {
    gen_config_from_path!("tests/test_api.yml");

    let mut test_service =
        test::init_service(App::new().service(web::scope("/api").configure(autoroute_config)))
            .await;
    let req = test::TestRequest::delete().uri("/api/foo").to_request();

    let resp = test::call_service(&mut test_service, req).await;
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[actix_rt::test]
async fn test_patch() {
    gen_config_from_path!("tests/test_api.yml");

    let mut test_service =
        test::init_service(App::new().service(web::scope("/api").configure(autoroute_config)))
            .await;
    let req = test::TestRequest::patch().uri("/api/foo").to_request();

    let resp = test::call_service(&mut test_service, req).await;
    assert_eq!(resp.status(), http::StatusCode::OK);
}

#[actix_rt::test]
async fn test_head() {
    gen_config_from_path!("tests/test_api.yml");

    let mut test_service =
        test::init_service(App::new().service(web::scope("/api").configure(autoroute_config)))
            .await;
    let req = test::TestRequest::with_uri("/api/foo")
        .method(Method::HEAD)
        .to_request();

    let resp = test::call_service(&mut test_service, req).await;
    assert_eq!(resp.status(), http::StatusCode::OK);
}
