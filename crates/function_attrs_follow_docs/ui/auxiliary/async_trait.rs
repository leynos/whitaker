// force-host
// no-prefer-dynamic
//! Minimal `async_trait`-style proc macro for UI fixtures.
#![crate_type = "proc-macro"]

extern crate proc_macro;

use proc_macro::{Delimiter, Group, Ident, Punct, Spacing, Span, TokenStream, TokenTree};

/// Inserts `#[async_trait]` before doc attributes in the annotated item.
#[proc_macro_attribute]
pub fn async_trait(_attr: TokenStream, item: TokenStream) -> TokenStream {
    transform_stream(item, Span::call_site())
}

fn transform_stream(stream: TokenStream, attribute_span: Span) -> TokenStream {
    let mut output = Vec::new();
    let mut iter = stream.into_iter().peekable();

    while let Some(token) = iter.next() {
        match &token {
            TokenTree::Group(group) => output.push(process_group_token(group, attribute_span)),
            TokenTree::Punct(punct) if punct.as_char() == '#' => {
                process_hash_token(&mut iter, attribute_span, &mut output);
                output.push(token);
            }
            _ => output.push(token),
        }
    }

    output.into_iter().collect()
}

fn process_group_token(group: &Group, attribute_span: Span) -> TokenTree {
    let delimiter = group.delimiter();
    let nested = transform_stream(group.stream(), attribute_span);
    let mut new_group = Group::new(delimiter, nested);
    new_group.set_span(group.span());
    TokenTree::Group(new_group)
}

fn process_hash_token(
    iter: &mut std::iter::Peekable<proc_macro::token_stream::IntoIter>,
    attribute_span: Span,
    output: &mut Vec<TokenTree>,
) {
    let Some(TokenTree::Group(group)) = iter.peek() else {
        return;
    };

    if should_insert_attribute(group) {
        output.extend(build_attribute(attribute_span));
    }
}

fn should_insert_attribute(group: &Group) -> bool {
    group.delimiter() == Delimiter::Bracket && is_doc_attribute(group.stream())
}

fn is_doc_attribute(stream: TokenStream) -> bool {
    matches!(
        stream.into_iter().next(),
        Some(TokenTree::Ident(ident)) if ident.to_string() == "doc"
    )
}

fn build_attribute(span: Span) -> Vec<TokenTree> {
    let mut hash = Punct::new('#', Spacing::Alone);
    hash.set_span(span);

    let mut inner = TokenStream::new();
    inner.extend([TokenTree::Ident(Ident::new("async_trait", span))]);

    let mut group = Group::new(Delimiter::Bracket, inner);
    group.set_span(span);

    vec![TokenTree::Punct(hash), TokenTree::Group(group)]
}
