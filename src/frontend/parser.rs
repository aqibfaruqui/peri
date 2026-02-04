use crate::frontend::ast;
use chumsky::prelude::*;
use chumsky::Parser;

pub fn parse(source_code: &str) -> Result<ast::Program, Vec<chumsky::error::Simple<'_, char>>> {
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
    let semicolon = just(';').padded();

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

        let return_stmt = text::keyword("return").padded()
            .ignore_then(expr.clone())
            .then_ignore(just(';').padded())
            .map(|expr| ast::Statement::Return { expr });

        if_stmt
            .or(while_stmt)
            .or(let_stmt)
            .or(assign_stmt)
            .or(return_stmt)
            .or(expr_stmt)
    });

    /*
     * Peripheral Declaration Parser
     * peripheral Timer at 0x4000_0000 {
     *     states: Off, On;
     *     initial: Off;
     *     registers u32 {
     *         CTRL at 0x00;
     *         COUNT at 0x04;
     *     }
     * }
     */
    
    // Parse hex number with optional underscores: 0x4000_0000
    let hex_digit_or_underscore = one_of("0123456789abcdefABCDEF_");
    let hex_num = just("0x")
        .ignore_then(
            hex_digit_or_underscore
                .repeated()
                .at_least(1)
                .to_slice()
                .map(|s: &str| {
                    let cleaned = s.replace('_', "");
                    u32::from_str_radix(&cleaned, 16).unwrap()
                })
        )
        .padded();
    
    let reg_type = text::keyword("u8").to(ast::RegisterType::U8)
        .or(text::keyword("u16").to(ast::RegisterType::U16))
        .or(text::keyword("u32").to(ast::RegisterType::U32))
        .padded();
    
    let register = ident.clone()
        .then_ignore(text::keyword("at").padded())
        .then(hex_num.clone())
        .then_ignore(semicolon.clone())
        .map(|(name, offset)| ast::Register { name, offset });
    
    let register_block = text::keyword("registers").padded()
        .ignore_then(reg_type)
        .then(
            register
                .repeated()
                .collect()
                .delimited_by(just('{').padded(), just('}').padded())
        )
        .map(|(reg_type, registers)| ast::RegisterBlock { reg_type, registers });
    
    let peripheral = text::keyword("peripheral").padded()
        .ignore_then(ident.clone())
        .then(
            text::keyword("at").padded()
                .ignore_then(hex_num.clone())
                .or_not()
        )
        .then_ignore(just('{').padded())
        .then_ignore(text::keyword("states").padded())
        .then_ignore(just(':').padded())
        .then(
            ident.clone()
                .separated_by(comma.clone())
                .at_least(1)
                .collect::<Vec<String>>()
        )
        .then_ignore(semicolon)
        .then_ignore(text::keyword("initial").padded())
        .then_ignore(just(':').padded())
        .then(ident.clone())
        .then_ignore(semicolon)
        .then(
            register_block
                .repeated()
                .collect()
        )
        .then_ignore(just('}').padded())
        .map(|((((name, base_address), states), initial), register_blocks)| ast::Peripheral {
            name,
            base_address,
            states,
            initial,
            register_blocks,
        });

    /*
     * Typestate Signature Parser (optional)
     * :: Peripheral<InputState> -> Peripheral<OutputState>
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
    let type_label = text::keyword("i32")
        .to(ast::Type::I32)
        .padded();

    let argument = ident
        .then_ignore(just(':')).padded()
        .then(type_label);

    let function = text::keyword("fn").padded()
        .ignore_then(ident)
        .then(
            argument
                .separated_by(comma)
                .allow_trailing()
                .collect()
                .delimited_by(just('(').padded(), just(')').padded()),
        )
        .then(signature.or_not())
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

    /* Program Parser: peripherals first, then functions */
    peripheral
        .padded()
        .repeated()
        .collect()
        .then(
            function
                .padded()
                .repeated()
                .collect()
        )
        .map(|(peripherals, functions)| ast::Program { peripherals, functions })
        .then_ignore(end())
}