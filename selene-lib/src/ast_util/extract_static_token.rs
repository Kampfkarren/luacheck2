use full_moon::{ast, tokenizer::TokenReference};

/// Given an expression that is a single token (like true), will return the one token.
/// Given one with parentheses, (like (true)), will return the token inside the parentheses.
pub fn extract_static_token(expression: &ast::Expression) -> Option<&TokenReference> {
    #[cfg_attr(
        feature = "force_exhaustive_checks",
        deny(non_exhaustive_omitted_patterns)
    )]
    match expression {
        ast::Expression::BinaryOperator { .. } | ast::Expression::UnaryOperator { .. } => None,

        ast::Expression::Parentheses { expression, .. } => extract_static_token(expression),

        ast::Expression::Number(token)
        | ast::Expression::String(token)
        | ast::Expression::Symbol(token) => Some(token),

        _ => None,
    }
}
