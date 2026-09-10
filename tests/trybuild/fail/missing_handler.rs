autoroute::gen_config_from!(
    r#"
openapi: 3.0.0
info:
  title: Test API
  version: 0.0.1
paths:
  /foo:
    get:
      responses:
        '200':
          description: success
"#
);

fn main() {}
