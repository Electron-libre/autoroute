autoroute::gen_config_from!(
    r##"
openapi: 3.0.0
x-autoroute-validate: true
paths:
  /foo:
    post:
      x-autoroute-handler: h
      requestBody:
        content:
          application/json:
            schema: {$ref: "#/components/schemas/Missing"}
"##
);

fn main() {}
