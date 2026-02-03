use crate::frontend::ast;
use crate::ir::{VirtualRegister, Instruction, Op};
use crate::ir::cfg::{CFG, BlockId, Terminator};
use std::collections::HashMap;

struct Context {
    vars: HashMap<String, VirtualRegister>,
    cfg: CFG,
    current_block: BlockId,
    next_register: usize,
}

impl Context {
    fn new() -> Self {
        let mut cfg = CFG::new();
        let entry = cfg.add_block();
        
        Self {
            vars: HashMap::new(),
            cfg,
            current_block: entry,
            next_register: 0,
        }
    }

    fn new_register(&mut self) -> VirtualRegister {
        let r = VirtualRegister { id: self.next_register };
        self.next_register += 1;
        r
    }

    fn new_block(&mut self) -> BlockId {
        self.cfg.add_block()
    }

    fn switch_to_block(&mut self, block: BlockId) {
        self.current_block = block;
    }

    fn emit(&mut self, instr: Instruction) {
        self.cfg.block_mut(self.current_block).push(instr);
    }

    fn set_terminator(&mut self, t: Terminator) {
        self.cfg.block_mut(self.current_block).set_terminator(t);
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

pub fn lower(prog: &ast::Program) -> Vec<(String, CFG)> {
    let mut lowered_functions = Vec::new();
    for func in &prog.functions {
        let cfg = lower_function(func);
        lowered_functions.push((func.name.clone(), cfg));
    }
    lowered_functions
}

fn lower_function(func: &ast::Function) -> CFG {
    let mut ctx = Context::new();

    for (i, (name, _type)) in func.args.iter().enumerate() {
        let reg = ctx.new_register();
        ctx.vars.insert(name.clone(), reg);
        ctx.emit(Instruction::new(
            Op::MovArg(i), 
            Some(reg),
            vec![]
        ));
    }

    for stmt in &func.body {
        lower_statement(&mut ctx, stmt);
    }

    if matches!(ctx.cfg.block(ctx.current_block).terminator, Terminator::None) {
        ctx.set_terminator(Terminator::Return(None));
    }

    ctx.cfg
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
            
            ctx.emit(Instruction::new(
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
            
            let then_bb = ctx.new_block();
            let else_bb = ctx.new_block();
            let merge_bb = ctx.new_block();
            
            // Current block branches based on condition
            ctx.set_terminator(Terminator::Branch {
                cond: cond_reg,
                then_block: then_bb,
                else_block: else_bb,
            });
            
            // Emit then block
            ctx.switch_to_block(then_bb);
            for s in then_block {
                lower_statement(ctx, s);
            }
            // If then block doesn't have a terminator, jump to merge
            if matches!(ctx.cfg.block(ctx.current_block).terminator, Terminator::None) {
                ctx.set_terminator(Terminator::Jump(merge_bb));
            }
            
            // Emit else block
            ctx.switch_to_block(else_bb);
            for s in else_block {
                lower_statement(ctx, s);
            }
            // If else block doesn't have a terminator, jump to merge
            if matches!(ctx.cfg.block(ctx.current_block).terminator, Terminator::None) {
                ctx.set_terminator(Terminator::Jump(merge_bb));
            }
            
            // Continue in merge block
            ctx.switch_to_block(merge_bb);
        }

        ast::Statement::While { cond, body } => {
            let header_bb = ctx.new_block();
            let body_bb = ctx.new_block();
            let exit_bb = ctx.new_block();
            
            // Current block jumps to loop header
            ctx.set_terminator(Terminator::Jump(header_bb));
            
            // Header evaluates condition and branches
            ctx.switch_to_block(header_bb);
            let cond_reg = lower_expression(ctx, cond);
            ctx.set_terminator(Terminator::Branch {
                cond: cond_reg,
                then_block: body_bb,
                else_block: exit_bb,
            });
            
            // Body executes and loops back to header
            ctx.switch_to_block(body_bb);
            for s in body {
                lower_statement(ctx, s);
            }
            // If body doesn't have a terminator, jump back to header
            if matches!(ctx.cfg.block(ctx.current_block).terminator, Terminator::None) {
                ctx.set_terminator(Terminator::Jump(header_bb));
            }
            
            // Continue in exit block
            ctx.switch_to_block(exit_bb);
        }

        ast::Statement::Return { expr } => {
            let value_reg = lower_expression(ctx, expr);
            ctx.set_terminator(Terminator::Return(Some(value_reg)));
        }
    }
}

fn lower_expression(ctx: &mut Context, expr: &ast::Expr) -> VirtualRegister {
    match expr {
        ast::Expr::IntLit { value } => {
            let dest = ctx.new_register();
            ctx.emit(Instruction::new(
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
            ctx.emit(Instruction::new(
                Op::Call(name.clone()),
                Some(dest),
                arg_regs
            ));
            dest
        }
    }
}