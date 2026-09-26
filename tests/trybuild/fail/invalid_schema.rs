autoroute::gen_config_from!(
    r##"
openapi: 3.0.0
paths:
  /foo:
    get:
      x-autoroute-handler: h
      x-autoroute-validate: true
      parameters:
        - name: limit
          in: query
          schema: {type: 12}
"##
);

fn main() {}
