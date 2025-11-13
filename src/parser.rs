use crate::ast;
use chumsky::prelude::*;
use chumsky::Parser;

pub fn parse(source_code: &str) -> Result<ast::Program, Vec<chumsky::error::Simple<char>>> {
    parser()
        .parse(source_code)
        .into_result()
        .map_err(|errs| errs.into_iter().map(|e| e.into_simple()).collect())
    }

fn parser<'src>() -> impl Parser<'src, &'src str, ast::Program, extra::Err<Simple<'src, char>>> {
    // All of our 'atoms' (like identifiers, keywords, symbols)
    // are '.padded()' to ignore whitespace around them.
    let ident = text::ident().padded();
    let comma = just(',').padded();

    /* Expression Parser */
    let expr = recursive(|expr| {
        // An expression atom is either a Variable or a FnCall
        // (change in line with ast.rs [Expr enum])

        let fn_call = ident
            .then(
                expr.clone()
                    .separated_by(comma)
                    .allow_trailing()
                    .collect()
                    .delimited_by(just('(').padded(), just(')').padded()),
            )
            .map(|(name, args)| ast::Expr::FnCall { name, args });

        let var = ident
            .map(|name: String| ast::Expr::Variable { name });

        fn_call.or(var)
    });

    /* Statement Parser */
    let statement = {
        // A statement atom is either a Let or an Expr
        // (change in line with ast.rs [Statement enum])

        let let_stmt = text::keyword("let").padded()
            .ignore_then(ident)
            .then_ignore(just('=').padded())
            .then(expr.clone())
            .then_ignore(just(';').padded())
            .map(|(var_name, value)| ast::Statement::Let { var_name, value });

        let expr_stmt = expr.clone()
            .then_ignore(just(';').padded())
            .map(|expr| ast::Statement::Expr { expr });

        let_stmt.or(expr_stmt)
    };

    /* Function Parser */
    // 'fn main(arg1, arg2) -> String { ...statements... }'
    let function = text::keyword("fn").padded()
        .ignore_then(ident)
        .then(
            ident
                .separated_by(comma)
                .allow_trailing()
                .collect()
                .delimited_by(just('(').padded(), just(')').padded()),
        )
        .then_ignore(just("->").padded())
        .then(ident)
        .then(
            statement
                .repeated()
                .collect()
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map(
            |(((name, args), return_type), body)| ast::Function {
                name,
                args,
                return_type,
                body,
            },
        );

    /* Program Parser */
    function
        .padded()
        .repeated()
        .collect()
        .map(|functions| ast::Program { functions })
        .then_ignore(end());
}