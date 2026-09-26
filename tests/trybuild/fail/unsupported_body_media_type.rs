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
          text/plain:
            schema: {type: string}
"##
);

fn main() {}
