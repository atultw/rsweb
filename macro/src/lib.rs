#![allow(unused_imports)]
extern crate proc_macro;
use ::proc_macro::TokenStream;
use ::proc_macro2::{
    Span,
    TokenStream as TokenStream2,
};
use ::quote::{
    quote,
    quote_spanned,
    ToTokens,
};
use ::syn::{*,
            parse::{Parse, Parser, ParseStream},
            punctuated::Punctuated,
            spanned::Spanned,
            Result,
};

#[proc_macro_derive(Fields)] pub
fn rule_system_derive (input: TokenStream)
                       -> TokenStream
{
    let ast = parse_macro_input!(input as _);
    TokenStream::from(match impl_my_trait(ast) {
        | Ok(it) => it,
        | Err(err) => err.to_compile_error(),
    })
}

fn impl_my_trait (ast: DeriveInput)
                  -> Result<TokenStream2>
{Ok({
    let name = ast.ident;
    let fields = match ast.data {
        | Data::Enum(DataEnum { enum_token: token::Enum { span }, .. })
        | Data::Union(DataUnion { union_token: token::Union { span }, .. })
        => {
            return Err(Error::new(
                span,
                "Expected a `struct`",
            ));
        },

        | Data::Struct(DataStruct { fields: Fields::Named(it), .. })
        => it,

        | Data::Struct(_)
        => {
            return Err(Error::new(
                Span::call_site(),
                "Expected a `struct` with named fields",
            ));
        },
    };

    let data_expanded_members = fields.named.into_iter().map(|field| {
        let field_name = field.ident.expect("Unreachable");
        let span = field_name.span();
        let field_name_stringified =
            LitStr::new(&field_name.to_string(), span)
            ;
        quote_spanned! { span=>
            Field {
                name: #field_name_stringified.to_string(),
                is_bool: false,
                is_num: false,
            }
        }
    });
    quote! {
        impl Fields for #name {
            fn fields() -> Vec<Field> {
                vec![
                    #(#data_expanded_members ,)*
                ]
            }
        }
    }
})}
