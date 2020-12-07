# Autoroute

Specification as a single source of truth for your rust http/json API.

Yet support ActixWeb framework. A pluggable adapter support is planned for the other web frameworks support.

# Usage

This is a work in progress API is subject to change and will follow semver.

1. Add `autoroute` to your Config.yml. Yet no crate has been released use the git master branch.
2. Add a valid OpenAPI 3 specification file to your sources.
3. Add "x-autoroute-handler" extension to open api operations with the name of your handler function as value.
4. use the `gen_config_from_path` proc macro to generate the `autoroute_config` function.
5. pass the `autoroute_config` function to your `actix_web::web::Scope.configure()`

For more illustrative documentation have a look the `tests/`.

# Done

* Automatic route configuration from an open-api v3 specification for Actix scope
* URL reflection.

# TODO

* Optional parameter validation against JSON schema within a middleware.
* Allow including api version in URL through config.
* Optional endpoint to expose the API specification.
