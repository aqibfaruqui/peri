use crate::frontend::ast;
use crate::ir::{VirtualRegister, Instruction, Op};
use std::collections::HashMap;

struct Context {
    vars: HashMap<String, VirtualRegister>,
    instructions: Vec<Instruction>,
    next_register: usize,
    label_counter: usize,
}

impl Context {
    fn new() -> Self {
        Self {
            vars: HashMap::new(),
            instructions: Vec::new(),
            next_register: 0,
            label_counter: 0,
        }
    }

    fn new_register(&mut self) -> VirtualRegister {
        let r = VirtualRegister { id: self.next_register };
        self.next_register += 1;
        r
    }

    // TODO: Fix labels to use same number on different parts of an If/While
    fn new_label(&mut self, suffix: &str) -> String {
        let label = format!(".L{}_{}", self.label_counter, suffix);
        self.label_counter += 1;
        label
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

    for (i, (name, _type)) in func.args.iter().enumerate() {
        let reg = ctx.new_register();
        ctx.vars.insert(name.clone(), reg);
        ctx.instructions.push(Instruction::new(
            Op::MovArg(i), 
            Some(reg),
            vec![]
        ));
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

        ast::Statement::If { cond, then_block, else_block } => {
            let cond_reg = lower_expression(ctx, cond);
            let label_if = ctx.new_label("if");
            let label_else = ctx.new_label("else");
            let label_end = ctx.new_label("end");
            
            ctx.instructions.push(Instruction::new(Op::Label(label_if), None, vec![]));
            ctx.instructions.push(Instruction::new(
                Op::BranchIfFalse(label_else.clone()), 
                None, 
                vec![cond_reg]
            ));
            for s in then_block { lower_statement(ctx, s); }
            ctx.instructions.push(Instruction::new(Op::Jump(label_end.clone()), None, vec![]));
            
            ctx.instructions.push(Instruction::new(Op::Label(label_else), None, vec![]));
            for s in else_block { lower_statement(ctx, s); }
            ctx.instructions.push(Instruction::new(Op::Label(label_end), None, vec![]));
        }

        ast::Statement::While { cond, body } => {
            let cond_reg = lower_expression(ctx, cond);
            let label_while = ctx.new_label("while");
            let label_end = ctx.new_label("end");

            ctx.instructions.push(Instruction::new(Op::Label(label_while.clone()), None, vec![]));
            ctx.instructions.push(Instruction::new(
                Op::BranchIfFalse(label_end.clone()),
                None,
                vec![cond_reg]
            ));
            for s in body { lower_statement(ctx, s); }
            ctx.instructions.push(Instruction::new(Op::Jump(label_while), None, vec![]));
            ctx.instructions.push(Instruction::new(Op::Label(label_end), None, vec![]));
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