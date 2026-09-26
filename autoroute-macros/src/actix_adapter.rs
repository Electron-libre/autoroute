use proc_macro2::{Ident, Punct, Spacing, Span, TokenStream};
use quote::{ToTokens, TokenStreamExt, quote};

use crate::spec::{
    HandlerIdentifier, Method, Operation, Operations, Parameter, ParameterLocation, Path, Spec,
    Validation,
};

impl Path {
    fn to_config(&self) -> TokenStream {
        let url = &self.url;
        let mut routes = quote! {};
        routes.append_separated(self.operations.to_config(), Punct::new('.', Spacing::Joint));

        let mut resource = quote! { ::actix_web::web::resource(#url) };
        if let Some(resource_name) = &self.resource_name {
            let name = quote! { .name(#resource_name) };
            resource.append_all(name);
        }

        let validations: Vec<TokenStream> = self
            .operations
            .0
            .iter()
            .filter_map(|operation| {
                let validation = operation.validation.as_ref()?;
                Some(validation.to_config(&operation.method))
            })
            .collect();
        if !validations.is_empty() {
            resource.append_all(quote! {
                .wrap(::autoroute::validation::Validation::new(vec![#(#validations),*]))
            });
        }

        let service = quote! {
            cfg.service(#resource.#routes)
        };
        service
    }
}

impl ToTokens for Method {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        let str_rep = Ident::new(&self.to_string(), Span::call_site());
        let method = quote! { ::actix_web::http::Method::#str_rep };
        tokens.append_all(method);
    }
}

impl Operations {
    fn to_config(&self) -> Vec<TokenStream> {
        self.0.iter().map(Operation::to_config).collect()
    }
}

impl Operation {
    fn to_config(&self) -> TokenStream {
        let method = &self.method;

        let handler = &self.handler;
        quote! { route(::actix_web::web::method(#method).to(#handler)) }
    }
}

impl Validation {
    fn to_config(&self, method: &Method) -> TokenStream {
        let method = method.to_string();
        let parameters = self.parameters.iter().map(Parameter::to_config);
        let body = self.body.as_ref().map(|body| {
            let required = body.required;
            let schema = body.schema.to_string();
            quote! { .with_body(::autoroute::validation::BodySpec::new(#required, #schema)) }
        });
        quote! {
            ::autoroute::validation::OperationValidation::new(#method, vec![#(#parameters),*])
                #body
        }
    }
}

impl Parameter {
    fn to_config(&self) -> TokenStream {
        let name = &self.name;
        let location = match self.location {
            ParameterLocation::Path => quote! { Path },
            ParameterLocation::Query => quote! { Query },
            ParameterLocation::Header => quote! { Header },
            ParameterLocation::Cookie => quote! { Cookie },
        };
        let required = self.required;
        let style = self
            .style
            .as_ref()
            .map_or_else(|| quote! { None }, |style| quote! { Some(#style) });
        let explode = self
            .explode
            .map_or_else(|| quote! { None }, |explode| quote! { Some(#explode) });
        let schema = self.schema.to_string();
        quote! {
            ::autoroute::validation::ParameterSpec::new(
                #name,
                ::autoroute::validation::Location::#location,
                #required,
                #style,
                #explode,
                #schema,
            )
        }
    }
}

impl ToTokens for HandlerIdentifier {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        Ident::new(self.0.as_str(), Span::call_site()).to_tokens(tokens);
    }
}

pub(crate) fn gen_config(spec: Spec) -> TokenStream {
    let paths = spec.paths.0.into_iter().map(|path| path.to_config());
    let mut configs = quote!();
    configs.append_terminated(paths, Punct::new(';', Spacing::Joint));
    let services = quote! {

        /// The config function to use with `actix_web::Scope::configure()`
        fn autoroute_config(cfg: &mut ::actix_web::web::ServiceConfig) {
            #configs
        }
    };
    services
}
