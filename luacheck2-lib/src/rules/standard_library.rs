use super::{super::standard_library::*, *};
use std::convert::Infallible;

use full_moon::{
    ast::{self, Ast},
    node::Node,
    tokenizer::{Symbol, TokenType},
    visitors::Visitor,
};

pub struct StandardLibraryLint;

impl Rule for StandardLibraryLint {
    type Config = ();
    type Error = Infallible;

    fn new(_: Self::Config) -> Result<Self, Self::Error> {
        Ok(StandardLibraryLint)
    }

    fn pass(&self, ast: &Ast, context: &Context) -> Vec<Diagnostic> {
        let mut visitor = StandardLibraryVisitor {
            diagnostics: Vec::new(),
            standard_library: &context.standard_library,
        };

        visitor.visit_ast(ast);

        visitor.diagnostics
    }

    fn severity(&self) -> Severity {
        Severity::Error
    }

    fn rule_type(&self) -> RuleType {
        RuleType::Correctness
    }
}

fn name_path_from_prefix_suffix<'a, 'ast, S: Iterator<Item = &'a ast::Suffix<'ast>>>(
    prefix: &'a ast::Prefix<'ast>,
    suffixes: S,
) -> Option<Vec<String>> {
    if let ast::Prefix::Name(ref name) = prefix {
        let mut names = Vec::new();
        names.push(name.to_string());

        for suffix in suffixes {
            if let ast::Suffix::Index(index) = suffix {
                if let ast::Index::Dot { name, .. } = index {
                    names.push(name.to_string());
                } else {
                    return None;
                }
            }
        }

        Some(names)
    } else {
        None
    }
}

fn name_path<'a, 'ast>(expression: &'a ast::Expression<'ast>) -> Option<Vec<String>> {
    if let ast::Expression::Value { value, .. } = expression {
        if let ast::Value::Var(var) = &**value {
            match var {
                ast::Var::Expression(expression) => {
                    name_path_from_prefix_suffix(expression.prefix(), expression.iter_suffixes())
                }

                ast::Var::Name(name) => Some(vec![name.to_string()]),
            }
        } else {
            None
        }
    } else {
        None
    }
}

// Returns the argument type of the expression if it can be constantly resolved
// Otherwise, returns None
// Only attempts to resolve constants
fn get_argument_type(expression: &ast::Expression) -> Option<ArgumentType> {
    match expression {
        ast::Expression::Parentheses { expression, .. } => get_argument_type(expression),

        ast::Expression::UnaryOperator { unop, expression } => {
            match unop {
                // CAVEAT: If you're overriding __len on a userdata and then making it not return a number
                // ...sorry, but I don't care about your code :)
                ast::UnOp::Hash(_) => Some(ArgumentType::Number),
                ast::UnOp::Minus(_) => get_argument_type(expression),
                ast::UnOp::Not(_) => Some(ArgumentType::Bool),
            }
        }

        ast::Expression::Value { binop: rhs, value } => {
            let base = match &**value {
                ast::Value::Function(_) => Some(ArgumentType::Function),
                ast::Value::FunctionCall(_) => None,
                ast::Value::Number(_) => Some(ArgumentType::Number),
                ast::Value::ParseExpression(expression) => get_argument_type(expression),
                ast::Value::String(_) => Some(ArgumentType::String),
                ast::Value::Symbol(symbol) => match *symbol.token_type() {
                    TokenType::Symbol { symbol } => match symbol {
                        Symbol::False => Some(ArgumentType::Bool),
                        Symbol::True => Some(ArgumentType::Bool),
                        Symbol::Nil => Some(ArgumentType::Nil),
                        _ => unreachable!(),
                    },

                    _ => unreachable!(),
                },
                ast::Value::TableConstructor(_) => Some(ArgumentType::Table),
                ast::Value::Var(_) => None,
            };

            if let Some(rhs) = rhs {
                // Nearly all of these will return wrong results if you have a non-idiomatic metatable
                // I intentionally omitted common metamethod re-typings, like __mul
                match rhs.bin_op() {
                    ast::BinOp::Caret(_) => Some(ArgumentType::Number),

                    ast::BinOp::GreaterThan(_)
                    | ast::BinOp::GreaterThanEqual(_)
                    | ast::BinOp::LessThan(_)
                    | ast::BinOp::LessThanEqual(_)
                    | ast::BinOp::TwoEqual(_)
                    | ast::BinOp::TildeEqual(_) => Some(ArgumentType::Bool),

                    // Basic types will often re-implement these (e.g. Roblox's Vector3)
                    ast::BinOp::Plus(_)
                    | ast::BinOp::Minus(_)
                    | ast::BinOp::Star(_)
                    | ast::BinOp::Slash(_) => base,

                    ast::BinOp::Percent(_) => Some(ArgumentType::Number),

                    ast::BinOp::TwoDots(_) => Some(ArgumentType::String),

                    ast::BinOp::And(_) | ast::BinOp::Or(_) => {
                        // We could potentially support union types here
                        // Or even just produce one type if both the left and right sides can be evaluated
                        // But for now, the evaluation just isn't smart enough to where this would be practical
                        None
                    }
                }
            } else {
                base
            }
        }
    }
}

pub struct StandardLibraryVisitor<'std> {
    standard_library: &'std StandardLibrary,
    diagnostics: Vec<Diagnostic>,
}

// TODO: Test shadowing
impl Visitor<'_> for StandardLibraryVisitor<'_> {
    fn visit_function_call(&mut self, call: &ast::FunctionCall) {
        let mut suffixes: Vec<&ast::Suffix> = call.iter_suffixes().collect();
        let call_suffix = suffixes.pop().unwrap();

        let name_path = match name_path_from_prefix_suffix(call.prefix(), suffixes.into_iter()) {
            Some(name_path) => name_path,
            None => return,
        };

        let field = match self.standard_library.find_global(&name_path) {
            Some(field) => field,
            None => return,
        };

        let arguments = match &field {
            standard_library::Field::Function(arguments) => arguments,
            _ => {
                unimplemented!("calling a property/table");
            }
        };

        // TODO: Support method calling
        match call_suffix {
            ast::Suffix::Call(call) => {
                if let ast::Call::AnonymousCall(args) = call {
                    let mut argument_types = Vec::new();

                    match args {
                        ast::FunctionArgs::Parentheses { arguments, .. } => {
                            for argument in arguments {
                                argument_types
                                    .push((argument.range().unwrap(), get_argument_type(argument)));
                            }
                        }

                        ast::FunctionArgs::String(token) => {
                            argument_types
                                .push((token.range().unwrap(), Some(ArgumentType::String)));
                        }

                        ast::FunctionArgs::TableConstructor(table) => {
                            argument_types
                                .push((table.range().unwrap(), Some(ArgumentType::Table)));
                        }
                    }

                    let mut expected_args = arguments.len();
                    let mut last_is_vararg = false;

                    if let Some(last) = arguments.last() {
                        if last.argument_type == ArgumentType::Vararg {
                            if let Required::Required(message) = &last.required {
                                // Functions like math.ceil where not using the vararg is wrong
                                if arguments.len() > argument_types.len() {
                                    self.diagnostics.push(Diagnostic::new_complete(
                                        "standard_library_types",
                                        format!(
                                            // TODO: This message isn't great
                                            "standard library function `{}` requires use of the vararg",
                                            name_path.join("."),
                                        ),
                                        Label::from_node(call, None),
                                        message.iter().cloned().collect(),
                                        Vec::new(),
                                    ));
                                }
                            }

                            expected_args -= 1;
                            last_is_vararg = true;
                        }
                    }

                    if argument_types.len() != expected_args
                        && (!last_is_vararg || argument_types.len() < expected_args)
                    {
                        self.diagnostics.push(Diagnostic::new(
                            "standard_library_types",
                            format!(
                                // TODO: This message isn't great
                                "standard library function `{}` requires {} parameters, {} passed",
                                name_path.join("."),
                                expected_args,
                                argument_types.len(),
                            ),
                            Label::from_node(call, None),
                        ));
                    }

                    for ((range, passed_type), expected) in
                        argument_types.iter().zip(arguments.iter())
                    {
                        if expected.argument_type == ArgumentType::Vararg {
                            continue;
                        }

                        if let Some(passed_type) = passed_type {
                            if passed_type != &expected.argument_type {
                                self.diagnostics.push(Diagnostic::new(
                                    "standard_library_types",
                                    format!(
                                        // TODO: This message isn't great
                                        "standard library function `{}` requires {} parameters, {} passed",
                                        name_path.join("."),
                                        expected_args,
                                        argument_types.len(),
                                    ),
                                    Label::new_with_message(
                                        (range.0.bytes() as u32, range.1.bytes() as u32),
                                        format!("expected `{}`, received `{}`", expected.argument_type, passed_type),
                                    ),
                                ));
                            }
                        }
                    }
                }
            }

            _ => unreachable!(),
        };
    }
}

#[cfg(test)]
mod tests {
    use super::{super::test_util::test_lint, *};

    #[test]
    fn test_name_path() {
        let ast = full_moon::parse("local x = foo; local y = foo.bar.baz").unwrap();

        struct NamePathTestVisitor {
            paths: Vec<Vec<String>>,
        }

        impl Visitor<'_> for NamePathTestVisitor {
            fn visit_local_assignment(&mut self, node: &ast::LocalAssignment) {
                self.paths.push(
                    name_path(node.expr_list().into_iter().next().unwrap())
                        .expect("name_path returned None"),
                );
            }
        }

        let mut visitor = NamePathTestVisitor { paths: Vec::new() };

        visitor.visit_ast(&ast);

        assert_eq!(
            visitor.paths,
            vec![
                vec!["foo".to_owned()],
                vec!["foo".to_owned(), "bar".to_owned(), "baz".to_owned()],
            ]
        );
    }

    #[test]
    fn test_bad_call_signatures() {
        test_lint(
            StandardLibraryLint::new(()).unwrap(),
            "standard_library",
            "bad_call_signatures",
        );
    }
}