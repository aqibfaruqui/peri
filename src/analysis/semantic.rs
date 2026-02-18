use crate::frontend::ast;
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug)]
pub enum SemanticError {
    UndefinedVariable {
        func_name: String,
        var_name: String,
    },

    UndefinedFunction {
        func_name: String,
        called_from: String,
    },

    ArityMismatch {
        func_name: String,
        expected: usize,
        actual: usize,
        called_from: String,
    },

    DuplicateFunction {
        func_name: String,
    },
}

impl fmt::Display for SemanticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SemanticError::UndefinedVariable { func_name, var_name } => {
                write!(f, "Undefined variable '{}' in function '{}'", var_name, func_name)
            }

            SemanticError::UndefinedFunction { func_name, called_from } => {
                write!(f, "Undefined function '{}' called from '{}'", func_name, called_from)
            }

            SemanticError::ArityMismatch { func_name, expected, actual, called_from } => {
                write!(f, "Function '{}' expects {} argument(s) but {} provided, called from '{}'", func_name, expected, actual, called_from)
            }

            SemanticError::DuplicateFunction { func_name } => {
                write!(f, "Duplicate function definition '{}'", func_name)
            }
        }
    }
}

pub fn check(program: &ast::Program) -> Result<(), Vec<SemanticError>> {
    let mut errors = Vec::new();
    let mut func_signatures: HashMap<String, usize> = HashMap::new();
    let mut seen_functions: HashSet<String> = HashSet::new();

    // TODO: Check if seen_functions needed or func_signatures can be used
    for func in &program.functions {
        if !seen_functions.insert(func.name.clone()) {
            errors.push(SemanticError::DuplicateFunction {
                func_name: func.name.clone(),
            });
        }
        func_signatures.insert(func.name.clone(), func.args.len());
    }

    for func in &program.functions {
        check_function(func, &func_signatures, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn check_function(
    func: &ast::Function,
    func_signatures: &HashMap<String, usize>,
    errors: &mut Vec<SemanticError>,
) {
    let mut scope: HashSet<String> = HashSet::new();
    for (arg_name, _) in &func.args {
        scope.insert(arg_name.clone());
    }

    for stmt in &func.body {
        check_statement(stmt, &func.name, func_signatures, &mut scope, errors);
    }
}

fn check_statement(
    stmt: &ast::Statement,
    func_name: &str,
    func_signatures: &HashMap<String, usize>,
    scope: &mut HashSet<String>,
    errors: &mut Vec<SemanticError>,
) {
    match stmt {
        ast::Statement::Let { var_name, value } => {
            check_expr(value, func_name, func_signatures, scope, errors);
            scope.insert(var_name.clone());
        }

        ast::Statement::Assign { var_name, value } => {
            if !scope.contains(var_name) {
                errors.push(SemanticError::UndefinedVariable {
                    func_name: func_name.to_string(),
                    var_name: var_name.clone(),
                });
            }
            check_expr(value, func_name, func_signatures, scope, errors);
        }

        ast::Statement::Expr { expr } => {
            check_expr(expr, func_name, func_signatures, scope, errors);
        }

        ast::Statement::Return { expr } => {
            check_expr(expr, func_name, func_signatures, scope, errors);
        }

        ast::Statement::If { cond, then_block, else_block } => {
            check_expr(cond, func_name, func_signatures, scope, errors);

            let mut then_scope = scope.clone();
            for s in then_block {
                check_statement(s, func_name, func_signatures, &mut then_scope, errors);
            }

            let mut else_scope = scope.clone();
            for s in else_block {
                check_statement(s, func_name, func_signatures, &mut else_scope, errors);
            }
        }

        ast::Statement::While { cond, body } => {
            check_expr(cond, func_name, func_signatures, scope, errors);

            let mut body_scope = scope.clone();
            for s in body {
                check_statement(s, func_name, func_signatures, &mut body_scope, errors);
            }
        }

        ast::Statement::PeripheralWrite { value, .. } => {
            check_expr(value, func_name, func_signatures, scope, errors);
        }
    }
}

fn check_expr(
    expr: &ast::Expr,
    func_name: &str,
    func_signatures: &HashMap<String, usize>,
    scope: &HashSet<String>,
    errors: &mut Vec<SemanticError>,
) {
    match expr {
        ast::Expr::IntLit { .. } => {}

        ast::Expr::Variable { name } => {
            if !scope.contains(name) {
                errors.push(SemanticError::UndefinedVariable {
                    func_name: func_name.to_string(),
                    var_name: name.clone(),
                });
            }
        }

        ast::Expr::Binary { left, right, .. } => {
            check_expr(left, func_name, func_signatures, scope, errors);
            check_expr(right, func_name, func_signatures, scope, errors);
        }

        ast::Expr::Unary { operand, .. } => {
            check_expr(operand, func_name, func_signatures, scope, errors);
        }

        ast::Expr::FnCall { name, args } => {
            match func_signatures.get(name) {
                None => {
                    errors.push(SemanticError::UndefinedFunction {
                        func_name: name.clone(),
                        called_from: func_name.to_string(),
                    });
                }
                Some(&expected_arity) => {
                    if args.len() != expected_arity {
                        errors.push(SemanticError::ArityMismatch {
                            func_name: name.clone(),
                            expected: expected_arity,
                            actual: args.len(),
                            called_from: func_name.to_string(),
                        });
                    }
                }
            }

            for arg in args {
                check_expr(arg, func_name, func_signatures, scope, errors);
            }
        }

        ast::Expr::PeripheralRead { .. } => {}
    }
}
