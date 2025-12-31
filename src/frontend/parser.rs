use crate::frontend::ast;
use chumsky::prelude::*;
use chumsky::Parser;

pub fn parse(source_code: &str) -> Result<ast::Program, Vec<chumsky::error::Simple<char>>> {
    parser()
        .parse(source_code)
        .into_result()
}

fn parser<'src>() -> impl Parser<'src, &'src str, ast::Program, extra::Err<Simple<'src, char>>> {
    // All of our 'atoms' (like identifiers, keywords, symbols)
    // are '.padded()' to ignore whitespace around them.
    let ident = text::ident()
        .padded()
        .map(|s: &str| s.to_string());

    let int_lit = text::int(10)
        .map(|s: &str| s.parse::<i32>().unwrap())
        .padded();
        
    let comma = just(',').padded();

    /* 
     * Expression Parser 
     * An expression atom is an IntLit, FnCall or Variable
     */
    let expr = recursive(|expr| {

        let val = int_lit
            .map(|value: i32| ast::Expr::IntLit { value });

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

        val.or(fn_call).or(var)
    });

    /*
     * Statement Parser 
     * A statement atom is a Let, Assign, Expr, If or While
     */
    let statement = recursive(|statement| {

        let block = statement.clone()
            .repeated()
            .collect()
            .delimited_by(just('{').padded(), just('}').padded());

        let let_stmt = text::keyword("let").padded()
            .ignore_then(ident)
            .then_ignore(just('=').padded())
            .then(expr.clone())
            .then_ignore(just(';').padded())
            .map(|(var_name, value)| ast::Statement::Let { var_name, value });

        let assign_stmt = ident
            .then_ignore(just('=').padded())
            .then(expr.clone())
            .then_ignore(just(';').padded())
            .map(|(var_name, value)| ast::Statement::Assign { var_name, value });

        let expr_stmt = expr.clone()
            .then_ignore(just(';').padded())
            .map(|expr| ast::Statement::Expr { expr });

        let if_stmt = text::keyword("if").padded()
            .ignore_then(expr.clone())
            .then(block.clone())
            .then(
                text::keyword("else").padded()
                .ignore_then(block.clone())
                .or_not()
            )
            .map(|((cond, then_block), else_block)| ast::Statement::If {
                cond,
                then_block,
                else_block: else_block.unwrap_or_default()
            });

        let while_stmt = text::keyword("while").padded()
            .ignore_then(expr.clone())
            .then(block.clone())
            .map(|(cond, body)| ast::Statement::While { cond, body });

        if_stmt
            .or(while_stmt)
            .or(let_stmt)
            .or(assign_stmt)
            .or(expr_stmt)
    });

    /*
     * Typestate Signature Parser
     * :: Type<InputState> -> Type<OutputState>
     */
    let type_state = ident
        .then(ident.delimited_by(just('<'), just('>')));

    let signature = just("::").padded()
        .ignore_then(type_state)
        .then_ignore(just("->").padded())
        .then(type_state)
        .map(|((type_1, state_1), (_type_2, state_2))| ast::TypeState {
            peripheral: type_1,
            input_state: state_1,
            output_state: state_2
        });

    /* 
     * Function Parser 
     * 'fn func(arg1, arg2) :: Type<InputState> -> Type<OutputState> { 
     *      statements 
     *  }'
     */
    let function = text::keyword("fn").padded()
        .ignore_then(ident)
        .then(
            ident
                .separated_by(comma)
                .allow_trailing()
                .collect()
                .delimited_by(just('(').padded(), just(')').padded()),
        )
        .then(signature)
        .then(
            statement
                .repeated()
                .collect()
                .delimited_by(just('{').padded(), just('}').padded()),
        )
        .map(|(((name, args), signature), body)| ast::Function {
            name,
            args,
            signature,
            body,
        });

    /* Program Parser */
    function
        .padded()
        .repeated()
        .collect()
        .map(|functions| ast::Program { functions })
        .then_ignore(end())
}