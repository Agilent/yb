#![feature(proc_macro_diagnostic)]

use proc_macro::TokenStream;
use proc_macro2::Span;

use quote::{quote, ToTokens};
use syn::parse::{Parse, ParseStream};
use syn::{parse_macro_input, Expr, Lit, Token};

// Example:
// ```no_run
// clone_repos! {
//     "https://github.com/yoctoproject/poky.git" => "test/",
//     "https://github.com/openembedded/meta-openembedded.git" in parent_dir,
// };
//

struct MacroInput(Vec<CloneCommand>);

impl Parse for MacroInput {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut ret = vec![];
        while let Ok(r) = input.parse::<CloneCommand>() {
            ret.push(r);
            if input.peek(Token![,]) {
                input.parse::<Token![,]>().unwrap();
            } else {
                break;
            }
        }
        Ok(MacroInput(ret))
    }
}

enum ExprOrLit {
    Expr(Expr),
    Lit(Lit),
}

impl ToTokens for ExprOrLit {
    fn to_tokens(&self, tokens: &mut proc_macro2::TokenStream) {
        match self {
            Self::Expr(expr) => expr.to_tokens(tokens),
            Self::Lit(lit) => lit.to_tokens(tokens),
        }
    }
}

impl Parse for ExprOrLit {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        if let Ok(expr) = input.parse::<Expr>() {
            Ok(ExprOrLit::Expr(expr))
        } else if input.peek(Lit) {
            return Ok(ExprOrLit::Lit(input.parse().unwrap()));
        } else {
            unimplemented!();
        }
    }
}

struct CloneCommand {
    uri: Lit,
    parent_dir: Option<ExprOrLit>,
    directory: Option<ExprOrLit>,
}

impl Parse for CloneCommand {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let uri: Lit = input.parse()?;

        let mut directory = None;
        if input.peek(Token![=>]) {
            input.parse::<Token![=>]>().unwrap();
            directory = Some(input.parse::<ExprOrLit>()?);
        }

        let mut parent_dir = None;
        if input.peek(Token![in]) {
            input.parse::<Token![in]>().unwrap();
            parent_dir = Some(input.parse::<ExprOrLit>()?);
        }
        Ok(CloneCommand {
            uri,
            parent_dir,
            directory,
        })
    }
}

#[proc_macro]
pub fn clone_repos(input: TokenStream) -> TokenStream {
    let mut macro_input: MacroInput = parse_macro_input!(input);
    let input_len = macro_input.0.len();

    if input_len == 0 {
        Span::call_site()
            .unwrap()
            .error("Need at least one clone command")
            .emit();
        return TokenStream::new();
    }

    let clones = macro_input
        .0
        .drain(..)
        .map(|v| {
            let uri = v.uri;
            let parent_dir = v
                .parent_dir
                .map(|p| quote! {Some(std::path::PathBuf::from(#p))})
                .unwrap_or(quote! {None});
            let directory = v
                .directory
                .map(|p| quote! {Some(String::from(#p))})
                .unwrap_or(quote! {None});

            quote! {
                client.clone_in(#uri, #parent_dir, #directory)
            }
        })
        .collect::<Vec<_>>();

    let unwraps = (0..input_len)
        .map(|v| {
            let v = syn::Index::from(v);
            quote! {
                results.#v.expect("RPC failed").expect("service error");
            }
        })
        .collect::<Vec<_>>();

    let ret = quote! {
        use concurrent_git_pool::PoolHelper;
        let client = PoolHelper::connect_or_local().await.expect("unable to establish PoolHelper");

        let results = tokio::join!(
            #(#clones),*
        );

        #(#unwraps)*
    };

    TokenStream::from(ret)
}
