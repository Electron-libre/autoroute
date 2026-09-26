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

# Validation

Requests can be validated against the JSON schemas of the specification, by an Actix middleware
generated for you. It is opt-in: set the `x-autoroute-validate` extension to `true` on the
specification root, on a path item or on an operation (the most specific one wins).

```yaml
openapi: 3.0.0
x-autoroute-validate: true # every operation...
paths:
  /foo/{fooId}:
    post:
      x-autoroute-handler: add_foo
      x-autoroute-validate: false # ...but this one
```

A request that does not comply is rejected with a `400 Bad Request`, all the problems being reported together:

```json
{"errors": [{"in": "query", "name": "limit", "pointer": "", "message": "1000 is greater than the maximum of 100"}]}
```

`in` is `path`, `query`, `header`, `cookie` or `body`, and `pointer` locates the problem inside the value.

* **Parameters** in `path`, `query`, `header` and `cookie` are validated against their `schema`. They are transported as
  strings, so they are converted to the type of their schema first (`"3"` is an integer for `type: integer`).
  Arrays and objects follow the `style` and `explode` of the parameter: `form`, `simple`, `spaceDelimited`,
  `pipeDelimited` and, for objects, `deepObject`.
* **The body** is validated when the `requestBody` has an `application/json` content. Requests carrying another
  `Content-Type` are rejected with a `415`. A body that is not `required` may be omitted. The handler still receives the
  body, so you can keep on using `web::Json`.
* Schemas are checked when your crate compiles: an invalid schema or an unknown `$ref` is a compile error.
  `$ref` to `components/schemas`, `components/parameters` and `components/requestBodies` are supported.
* Schemas are turned into JSON Schema draft 4 before validation: `nullable` becomes a type accepting `null`, and the
  documentation only keywords (`example`, `discriminator`, ...) are dropped.

Limits:

* Only local `$ref` are supported.
* A parameter must have a `schema`: `content` parameters cannot be validated.
* Only JSON bodies can be validated. An operation with another kind of body must opt out with
  `x-autoroute-validate: false`.
* The `label` and `matrix` styles are not supported, and the properties of an object parameter are converted only when
  they are scalars.
* With the `form` style, the properties of an exploded object parameter (`?offset=1&size=5`) are told from the other
  query parameters by the `properties` of its schema. Unknown keys are ignored, not rejected.
* The body is read entirely before the handler runs: its size is bounded by the `PayloadConfig` of your application,
  256 KiB by default. Raise it with `App::app_data(web::PayloadConfig::new(...))` if your payloads are bigger.
* The specification is read as a YAML/JSON document: the validation supports OpenAPI 3.0 (3.1 schemas are not converted).

The middleware lives in the `validation` feature of `autoroute`, enabled by default.

# Done

* Automatic route configuration from an open-api v3 specification for Actix scope
* URL reflection.
* Optional parameter and body validation against JSON schema within a middleware.

# TODO

* Allow including api version in URL through config.
* Optional endpoint to expose the API specification.
