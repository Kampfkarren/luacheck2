use super::*;
use crate::ast_util::scopes::ScopeManager;
use std::collections::HashSet;

use full_moon::ast::Ast;
use regex::Regex;
use serde::Deserialize;

#[derive(Clone, Deserialize)]
#[serde(default)]
pub struct UnscopedVariablesConfig {
    ignore_pattern: String,
}

impl Default for UnscopedVariablesConfig {
    fn default() -> Self {
        Self {
            ignore_pattern: "^_".to_owned(),
        }
    }
}

pub struct UnscopedVariablesLint {
    ignore_pattern: Regex,
}

impl Rule for UnscopedVariablesLint {
    type Config = UnscopedVariablesConfig;
    type Error = regex::Error;

    fn new(config: Self::Config) -> Result<Self, Self::Error> {
        Ok(UnscopedVariablesLint {
            ignore_pattern: Regex::new(&config.ignore_pattern)?,
        })
    }

    fn pass(&self, ast: &Ast, context: &Context) -> Vec<Diagnostic> {
        // ScopeManager repeats references, and I just don't want to fix it right now
        let mut read = HashSet::new();

        let mut diagnostics = Vec::new();
        let scope_manager = ScopeManager::new(ast);

        for (_, reference) in &scope_manager.references {
            if reference.resolved.is_none()
                && reference.write
                && !read.contains(&reference.identifier)
                && !self.ignore_pattern.is_match(&reference.name)
                && !context
                    .standard_library
                    .globals
                    .contains_key(&reference.name)
            {
                read.insert(reference.identifier);

                diagnostics.push(Diagnostic::new(
                    "unscoped_variables",
                    format!(
                        "`{}` is not declared locally, and will be available in every scope",
                        reference.name
                    ),
                    Label::new(reference.identifier),
                ));
            }
        }

        diagnostics
    }

    fn severity(&self) -> Severity {
        Severity::Warning
    }

    fn rule_type(&self) -> RuleType {
        RuleType::Complexity
    }
}

#[cfg(test)]
mod tests {
    use super::{super::test_util::*, *};

    #[test]
    fn test_unscoped_variables() {
        test_lint(
            UnscopedVariablesLint::new(UnscopedVariablesConfig::default()).unwrap(),
            "unscoped_variables",
            "unscoped_variables",
        );
    }
}
