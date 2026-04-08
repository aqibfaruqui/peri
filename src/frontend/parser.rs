use crate::frontend::ast::{self, TypeStateSet};
use chumsky::prelude::*;
use chumsky::pratt::*;
use chumsky::Parser;

pub fn parse(source_code: &str) -> Result<ast::Program, Vec<chumsky::error::Simple<'_, char>>> {
    parser()
        .parse(source_code)
        .into_result()
}

fn parser<'src>() -> impl Parser<'src, &'src str, ast::Program, extra::Err<Simple<'src, char>>> {
    let comment = just("//")    // TODO: Add support for block comments /* ... */
        .then(none_of('\n').repeated())
        .ignored();

    let ws = comment
        .or(text::whitespace().at_least(1).ignored())
        .repeated();

    let ident = text::ident()
        .padded_by(ws.clone())
        .map(|s: &str| s.to_string());

    let int_lit = text::int(10)
        .map(|s: &str| s.parse::<i32>().unwrap())
        .padded_by(ws.clone());

    let hex_digits = one_of("0123456789abcdefABCDEF_")
        .repeated()
        .at_least(1)
        .to_slice()
        .map(|s: &str| s.replace('_', ""));

    let hex_num = just("0x")
        .ignore_then(hex_digits.clone().map(|s| u32::from_str_radix(&s, 16).unwrap()))
        .padded_by(ws.clone());

    let hex_lit = just("0x")
        .ignore_then(hex_digits.map(|s| i32::from_str_radix(&s, 16).unwrap_or(0)))
        .padded_by(ws.clone())
        .map(|value| ast::Expr::IntLit { value });

    let bin_digits = one_of("01_")
        .repeated()
        .at_least(1)
        .to_slice()
        .map(|s: &str| s.replace('_', ""));

    let bin_lit = just("0b")
        .ignore_then(bin_digits.map(|s| i32::from_str_radix(&s, 2).unwrap_or(0)))
        .padded_by(ws.clone())
        .map(|value| ast::Expr::IntLit { value });

    let ctrl_char = just('\\').ignore_then(choice((
        just('n').to('\n'),
        just('t').to('\t'),
        just('r').to('\r'),
        just('\\').to('\\'),
        just('\'').to('\''),
        just('0').to('\0'),
    )));

    let char_lit = ctrl_char
        .or(none_of('\'' ))
        .delimited_by(just('\''), just('\'' ))
        .padded_by(ws.clone())
        .map(|c: char| ast::Expr::IntLit { value: c as i32 });

    let bool_lit = text::keyword("true").to(ast::Expr::IntLit { value: 1 })
        .or(text::keyword("false").to(ast::Expr::IntLit { value: 0 }))
        .padded_by(ws.clone());

    let comma = just(',').padded_by(ws.clone());
    let semicolon = just(';').padded_by(ws.clone());
    let equals = just('=').padded_by(ws.clone());

    /* Expression Parser */
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

        let peripheral_read = ident
            .then_ignore(just("::"))
            .then(ident)
            .map(|(peripheral, register)| ast::Expr::PeripheralRead { peripheral, register });

        let var = ident
            .map(|name: String| ast::Expr::Variable { name });

        let atom = char_lit.clone()
            .or(hex_lit.clone())
            .or(bin_lit.clone())
            .or(bool_lit.clone())
            .or(val)
            .or(fn_call)
            .or(peripheral_read)
            .or(var)
            .or(expr.clone().delimited_by(just('(').padded_by(ws.clone()), just(')').padded_by(ws.clone())))
            .padded_by(ws.clone());

        let unary_op = just('-').to(ast::UnaryOp::Neg)
            .or(just('!').to(ast::UnaryOp::Not))
            .or(just('~').to(ast::UnaryOp::BitNot))
            .padded_by(ws.clone());

        let unary = unary_op.repeated().foldr(atom, |op, expr| ast::Expr::Unary {
            op,
            operand: Box::new(expr),
        });

        unary.pratt((

            infix(left(10), just('*').padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Mul, left: Box::new(l), right: Box::new(r),
            }),
            infix(left(10), just('/').padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Div, left: Box::new(l), right: Box::new(r),
            }),
            infix(left(10), just('%').padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Mod, left: Box::new(l), right: Box::new(r),
            }),

            infix(left(9), just('+').padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Add, left: Box::new(l), right: Box::new(r),
            }),
            infix(left(9), just('-').padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Sub, left: Box::new(l), right: Box::new(r),
            }),

            infix(left(8), just("<<").padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Shl, left: Box::new(l), right: Box::new(r),
            }),
            infix(left(8), just(">>").padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Shr, left: Box::new(l), right: Box::new(r),
            }),

            infix(left(7), just("<=").padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Le, left: Box::new(l), right: Box::new(r),
            }),
            infix(left(7), just(">=").padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Ge, left: Box::new(l), right: Box::new(r),
            }),
            infix(left(7), just('<').padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Lt, left: Box::new(l), right: Box::new(r),
            }),
            infix(left(7), just('>').padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Gt, left: Box::new(l), right: Box::new(r),
            }),

            infix(left(6), just("==").padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Eq, left: Box::new(l), right: Box::new(r),
            }),
            infix(left(6), just("!=").padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Ne, left: Box::new(l), right: Box::new(r),
            }),

            infix(left(5), just('&').padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::BitAnd, left: Box::new(l), right: Box::new(r),
            }),

            infix(left(4), just('^').padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::BitXor, left: Box::new(l), right: Box::new(r),
            }),

            infix(left(3), just('|').padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::BitOr, left: Box::new(l), right: Box::new(r),
            }),

            infix(left(2), just("&&").padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::And, left: Box::new(l), right: Box::new(r),
            }),

            infix(left(1), just("||").padded(), |l, _, r, _| ast::Expr::Binary {
                op: ast::BinaryOp::Or, left: Box::new(l), right: Box::new(r),
            }),
        ))
    });

    /* Statement Parser  */
    let statement = recursive(|statement| {

        let block = statement.clone()
            .repeated()
            .collect()
            .delimited_by(just('{').padded_by(ws.clone()), just('}').padded_by(ws.clone()));

        let let_stmt = text::keyword("let").padded_by(ws.clone())
            .ignore_then(ident)
            .then_ignore(equals)
            .then(expr.clone())
            .then_ignore(just(';').padded_by(ws.clone()))
            .map(|(var_name, value)| ast::Statement::Let { var_name, value });

        let const_stmt = text::keyword("const").padded_by(ws.clone())
            .ignore_then(ident)
            .then_ignore(equals)
            .then(expr.clone())
            .then_ignore(just(';').padded_by(ws.clone()))
            .map(|(var_name, value)| ast::Statement::Const { var_name, value });

        let assign_stmt = ident
            .then_ignore(equals)
            .then(expr.clone())
            .then_ignore(just(';').padded_by(ws.clone()))
            .map(|(var_name, value)| ast::Statement::Assign { var_name, value });

        let expr_stmt = expr.clone()
            .then_ignore(just(';').padded_by(ws.clone()))
            .map(|expr| ast::Statement::Expr { expr });

        let if_stmt = text::keyword("if").padded_by(ws.clone())
            .ignore_then(expr.clone())
            .then(block.clone())
            .then(
                text::keyword("else").padded_by(ws.clone())
                .ignore_then(block.clone())
                .or_not()
            )
            .map(|((cond, then_block), else_block)| ast::Statement::If {
                cond,
                then_block,
                else_block: else_block.unwrap_or_default()
            });

        let while_stmt = text::keyword("while").padded_by(ws.clone())
            .ignore_then(expr.clone())
            .then(block.clone())
            .map(|(cond, body)| ast::Statement::While { cond, body });

        let return_stmt = text::keyword("return").padded_by(ws.clone())
            .ignore_then(expr.clone())
            .then_ignore(just(';').padded_by(ws.clone()))
            .map(|expr| ast::Statement::Return { expr });

        let peripheral_write_stmt = ident
            .then_ignore(just("::"))
            .then(ident)
            .then_ignore(equals)
            .then(expr.clone())
            .then_ignore(just(';').padded_by(ws.clone()))
            .map(|((peripheral, register), value)| ast::Statement::PeripheralWrite { 
                peripheral, 
                register, 
                value 
            });

        if_stmt
            .or(while_stmt)
            .or(const_stmt)
            .or(let_stmt)
            .or(peripheral_write_stmt)
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
    
    let reg_type = text::keyword("u8").to(ast::RegisterType::U8)
        .or(text::keyword("u16").to(ast::RegisterType::U16))
        .or(text::keyword("u32").to(ast::RegisterType::U32))
        .padded_by(ws.clone());
    
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

    let ts_label = just('!').padded_by(ws.clone())
        .ignore_then(text::ident().padded_by(ws.clone()))
        .map(|s: &str| format!("!{s}"))
        .or(text::ident().padded_by(ws.clone()).map(|s: &str| s.to_string()));

    let ts_set = ts_label.clone()
        .separated_by(just('&').padded_by(ws.clone()))
        .at_least(1)
        .collect::<Vec<String>>()
        .map(|labels| labels.into_iter().collect::<TypeStateSet>());

    let ts_set_vec = ts_set.clone()
        .separated_by(just('|').padded_by(ws.clone()))
        .at_least(1)
        .collect::<Vec<TypeStateSet>>();

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
        .then(
            text::keyword("typestate").padded_by(ws.clone())
                .ignore_then(ident.clone())
                .then_ignore(just('=').padded_by(ws.clone()))
                .then(ts_set_vec.clone())
                .then_ignore(semicolon.clone())
                .map(|(name, definition)| ast::TypeStateAlias { name, definition })
                .repeated()
                .collect::<Vec<ast::TypeStateAlias>>()
        )
        .then_ignore(just('}').padded())
        .map(|(((((name, base_address), states), initial), register_blocks), aliases)| ast::Peripheral {
            name,
            base_address,
            states,
            initial,
            register_blocks,
            aliases,
        });

    /*
     * Typestate Signature Parser
     *
     * Examples:
     *   P<Ready>             input = [{"Ready"}], output = {"Ready"}
     *   P<A | B>             input = [{"A"}, {"B"}]
     *   P<A & B>             input = [{"A","B"}]
     *   P<A | B & C>         input = [{"A"}, {"B","C"}]
     *   fn f() :: P<A | B> -> P<C & D>
     */

    let sig_input = ident.clone()
        .then(ts_set_vec.clone().delimited_by(
            just('<').padded_by(ws.clone()),
            just('>').padded_by(ws.clone()),
        ));

    let sig_output = ident.clone()
        .then(ts_set.clone().delimited_by(
            just('<').padded_by(ws.clone()),
            just('>').padded_by(ws.clone()),
        ));

    let type_param_bound = text::keyword("as").padded_by(ws.clone())
            .to(ast::BoundKind::As)
        .or(text::keyword("includes").padded_by(ws.clone())
            .to(ast::BoundKind::Includes))
        .then(
            just('!').padded_by(ws.clone())
                .ignore_then(text::ident().padded_by(ws.clone()))
                .map(|s: &str| format!("!{s}"))
                .or(text::ident().padded_by(ws.clone()).map(|s: &str| s.to_string()))
        )
        .or_not();

    let type_param = ident.clone()
        .then(type_param_bound)
        .map(|(name, opt)| match opt {
            None => ast::TypeParam { name, bound: String::new(), kind: ast::BoundKind::Includes },
            Some((kind, bound)) => ast::TypeParam { name, bound, kind },
        });

    let type_param_list = type_param
        .separated_by(just(',').padded_by(ws.clone()))
        .at_least(1)
        .collect::<Vec<ast::TypeParam>>()
        .delimited_by(just('<').padded_by(ws.clone()), just('>').padded_by(ws.clone()))
        .or_not()
        .map(|opt| opt.unwrap_or_default());

    /* 
     * Function Parser 
     * 'fn func(arg1, arg2) :: Type<InputState> -> Type<OutputState> { 
     *      statements 
     *  }'
     */
    let type_label = text::keyword("i32").to(ast::Type::I32)
        .or(text::keyword("u8").to(ast::Type::U8))
        .or(text::keyword("u16").to(ast::Type::U16))
        .or(text::keyword("u32").to(ast::Type::U32))
        .or(text::keyword("char").to(ast::Type::U8))
        .padded_by(ws.clone());

    let argument = ident
        .then_ignore(just(':')).padded()
        .then(type_label.clone());

    let signature_body = just("::").padded_by(ws.clone())
        .ignore_then(sig_input)
        .then_ignore(just("->").padded_by(ws.clone()))
        .then(sig_output)
        .map(|((periph, input_states), (_periph_out, output_state))| {
            ast::TypeState { peripheral: periph, type_params: vec![], input_states, output_state }
        });

    let function = text::keyword("fn").padded_by(ws.clone())
        .ignore_then(ident)
        .then(
            argument
                .separated_by(comma)
                .allow_trailing()
                .collect()
                .delimited_by(just('(').padded_by(ws.clone()), just(')').padded_by(ws.clone())),
        )
        .then(type_param_list)
        .then_ignore(just("->").padded_by(ws.clone()).then(type_label.clone()).or_not())
        .then(signature_body.or_not())
        .then(
            statement
                .repeated()
                .collect()
                .delimited_by(just('{').padded_by(ws.clone()), just('}').padded_by(ws.clone())),
        )
        .map(|((((name, args), type_params), sig_opt), body)| {
            let signature = sig_opt.map(|mut sig| { sig.type_params = type_params; sig });
            ast::Function { name, args, signature, body }
        });

    let global_const = text::keyword("const").padded_by(ws.clone())
        .ignore_then(ident.clone())
        .then_ignore(equals.clone())
        .then(expr.clone())
        .then_ignore(just(';').padded_by(ws.clone()));

    /* Program Parser: peripherals, global consts and functions */
    peripheral
        .padded_by(ws.clone())
        .repeated()
        .collect()
        .then(
            global_const
                .padded_by(ws.clone())
                .repeated()
                .collect::<Vec<(String, ast::Expr)>>()
        )
        .then(
            function
                .padded_by(ws.clone())
                .repeated()
                .collect()
        )
        .map(|((peripherals, constants), functions)| ast::Program { peripherals, constants, functions })
        .padded_by(ws.clone())
        .then_ignore(end())
}