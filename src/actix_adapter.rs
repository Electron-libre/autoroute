use proc_macro2::{Ident, Punct, Spacing, Span, TokenStream};
use quote::{quote, ToTokens, TokenStreamExt};

use crate::spec::{HandlerIdentifier, Method, Operation, Operations, Path, Spec};

impl Path {
    fn to_config(&self) -> TokenStream {
        let url = &self.url;
        let mut routes = quote! {};
        routes.append_separated(self.operations.to_config(), Punct::new('.', Spacing::Joint));
        let service = quote! {
            cfg.service(web::resource(#url).#routes)
        };
        service
    }
}

impl Operations {
    fn to_config(&self) -> Vec<TokenStream> {
        self.0.iter().map(|o| o.to_config()).collect()
    }
}

impl Operation {
    fn to_config(&self) -> TokenStream {
        let method = match self.method {
            Method::Get => quote! { GET },
            Method::Post => quote! { POST },
            Method::Put => quote! { PUT },
            Method::Delete => quote! { DELETE },
            Method::Patch => quote! { PATCH },
            Method::Head => quote! { HEAD },
            Method::Options => quote! { OPTIONS },
        };

        let handler = &self.handler;
        quote! { route(web::method(http::Method::#method).to(#handler)) }
    }
}

impl ToTokens for HandlerIdentifier {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        Ident::new(self.0.as_str(), Span::call_site()).to_tokens(tokens)
    }
}

pub(crate) fn gen_config(spec: Spec) -> TokenStream {
    let paths = spec.paths.0.into_iter().map(|path| path.to_config());
    let mut configs = quote!();
    configs.append_terminated(paths, Punct::new(';', Spacing::Joint));
    let services = quote! {

        /// The config function to use with `actix_web::Scope::configure()`
        fn autoroute_config(cfg: &mut actix_web::web::ServiceConfig) {
            #configs
        }
    };
    services
}
