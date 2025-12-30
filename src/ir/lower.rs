use crate::frontend::ast;
use crate::ir::{VirtualRegister, Instruction, Op};
use std::collections::HashMap;

struct Context {
    vars: HashMap<String, VirtualRegister>,
    instructions: Vec<Instruction>,
    next_register: usize,
}

impl Context {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
            instructions: Vec::new(),
            next_register: 0,
        }
    }

    fn new_register(&mut self) -> VirtualRegister {
        let r = VirtualRegister { id: self.next_register };
        self.next_register += 1;
        r
    }

    fn get_register(&self, name: &str) -> VirtualRegister {
        // TODO: Update to return Result<VirtualRegister, &'static str>
        // let var = match self.vars.get(name) {
        //     Some(v) => v,
        //     None => return Err("Didn't get a destination file path"),
        // };

        // Ok(var)
        *self.vars.get(name).expect(&format!("Variable {} not found", name))
    }
}

pub fn lower(prog: &ast::Program) -> Vec<(String, Vec<Instruction>)> {
    let mut lowered_functions = Vec::new();
    for func in &prog.functions {
        let instructions = lower_function(func);
        lowered_functions.push((func.name.clone(), instructions));
    }
    lowered_functions
}

fn lower_function(func: &ast::Function) -> Vec<Instruction> {
    let mut ctx = Context::new();

    // TODO: distinguish between virtual registers for function arguments and general use
    for arg in &func.args {
        let reg = ctx.new_register();
        ctx.vars.insert(arg.clone(), reg);
    }

    for stmt in &func.body {
        lower_statement(&mut ctx, stmt);
    }

    ctx.instructions.push(Instruction::new(Op::Ret, None, vec![]));
    ctx.instructions
}

fn lower_statement(ctx: &mut Context, stmt: &ast::Statement) {
    match stmt {
        ast::Statement::Let { var_name, value } => {
            let result_reg = lower_expression(ctx, value);
            ctx.vars.insert(var_name.clone(), result_reg);
        }

        ast::Statement::Assign { var_name, value } => {
            let value_reg = lower_expression(ctx, value);
            let target_reg = ctx.get_register(var_name);
            
            ctx.instructions.push(Instruction::new(
                Op::Mov,
                Some(target_reg),
                vec![value_reg]
            ));
        }

        ast::Statement::Expr { expr } => {
            lower_expression(ctx, expr);
        }
    }
}

fn lower_expression(ctx: &mut Context, expr: &ast::Expr) -> VirtualRegister {
    match expr {
        ast::Expr::IntLit { value } => {
            let dest = ctx.new_register();
            ctx.instructions.push(Instruction::new(
                Op::LoadImm(*value), 
                Some(dest), 
                vec![]
            ));
            dest
        }

        ast::Expr::Variable { name } => {
            ctx.get_register(name)
        }

        ast::Expr::FnCall { name, args } => {
            let mut arg_regs = Vec::new();
            for arg in args {
                arg_regs.push(lower_expression(ctx, arg));
            }
            
            let dest = ctx.new_register();
            ctx.instructions.push(Instruction::new(
                Op::Call(name.clone()),
                Some(dest),
                arg_regs
            ));
            dest
        }
    }
}