use super::*;
use std::convert::Infallible;

use full_moon::{
    ast::{self, Ast},
    visitors::Visitor,
};

pub struct CompareNanLint;

impl Rule for CompareNanLint {
    type Config = ();
    type Error = Infallible;

    fn new(_: Self::Config) -> Result<Self, Self::Error> {
        Ok(CompareNanLint)
    }

    fn pass(&self, ast: &Ast, _: &Context) -> Vec<Diagnostic> {
        let mut visitor = CompareNanVisitor {
            comparisons: Vec::new(),
        };

        visitor.visit_ast(&ast);

        visitor
            .comparisons
            .iter()
            .map(|comparisons| {
                Diagnostic::new_complete(
                    "compare_nan",
                    "comparing things to nan directly is not allowed".to_owned(),
                    Label::new(comparisons.range),
                    vec![format!(
                        "try: `{variable} {operator} {variable}` instead",
                        variable = comparisons.variable,
                        operator = comparisons.operator
                    )],
                    Vec::new(),
                )
            })
            .collect()
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn rule_type(&self) -> RuleType {
        RuleType::Correctness
    }
}

struct CompareNanVisitor {
    comparisons: Vec<Comparison>,
}

struct Comparison {
    variable: String,
    operator: String,
    range: (usize, usize),
}

fn value_is_zero(value: &ast::Value) -> bool {
    if let ast::Value::Number(token) = value {
        token.token().to_string() == "0"
    } else {
        false
    }
}

fn expression_is_nan(node: &ast::Expression) -> bool {
    if_chain::if_chain! {
        if let ast::Expression::Value { value, binop, .. } = node;
        if let Some(rhs) = binop;
        if let ast::BinOp::Slash(_) = rhs.bin_op();
        if let ast::Expression::Value {
            value: rhs_value, ..
        } = rhs.rhs();
        if value_is_zero(rhs_value) && value_is_zero(value);
        then {
            return true
        }
    }
    false
}

impl Visitor<'_> for CompareNanVisitor {
    fn visit_expression(&mut self, node: &ast::Expression) {
        if_chain::if_chain! {
            if let ast::Expression::Value {value, binop, ..} = node;
            if let ast::Value::Var(_) = value.as_ref();
            if let Some(rhs) = binop;
            then {
                let actual_op = rhs.bin_op();
                match actual_op {
                    ast::BinOp::TildeEqual(_) | ast::BinOp::TwoEqual(_) => {
                        if expression_is_nan(rhs.rhs()) {
                            let range = node.range().unwrap();
                            self.comparisons.push(
                                Comparison {
                                    variable: value.to_string().trim().to_owned(),
                                    operator: actual_op.to_string().trim().to_owned(),
                                    range: ((range.0.bytes(), range.1.bytes())),
                                }
                            );
                        }
                    },
                    _ => {},
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{super::test_util::test_lint, *};

    #[test]
    fn test_compare_nan_variables() {
        test_lint(
            CompareNanLint::new(()).unwrap(),
            "compare_nan",
            "compare_nan_variables",
        );
    }
    #[test]
    fn test_compare_nan_if() {
        test_lint(
            CompareNanLint::new(()).unwrap(),
            "compare_nan",
            "compare_nan_if",
        );
    }
}
