use crate::frontend::ast;
use crate::ir::{VirtualRegister, Instruction, Op};
use crate::ir::cfg::{CFG, BlockId, Terminator, Statement, Expr};
use std::collections::HashMap;

struct Context<'a> {
    vars: HashMap<String, VirtualRegister>,
    peripherals: &'a [ast::Peripheral],
    signatures: HashMap<String, &'a ast::TypeState>,
    cfg: CFG,
    current_block: BlockId,
    next_register: usize,
}

impl<'a> Context<'a> {
    fn new(peripherals: &'a [ast::Peripheral], signatures: HashMap<String, &'a ast::TypeState>) -> Self {
        let mut cfg = CFG::new();
        let entry = cfg.add_block();
        
        Self {
            vars: HashMap::new(),
            peripherals,
            signatures,
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

    fn add_block(&mut self) -> BlockId {
        self.cfg.add_block()
    }

    fn switch_to(&mut self, block: BlockId) {
        self.current_block = block;
    }

    fn emit_instr(&mut self, instr: Instruction) {
        self.cfg.block_mut(self.current_block).instructions.push(instr);
    }

    fn emit_stmt(&mut self, stmt: Statement) {
        self.cfg.block_mut(self.current_block).statements.push(stmt);
    }

    fn set_terminator(&mut self, term: Terminator) {
        self.cfg.block_mut(self.current_block).terminator = term;
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

    fn get_mmio_address(&self, peripheral_name: &str, register_name: &str) -> Option<u32> {
        for p in self.peripherals {
            if p.name == peripheral_name {
                let base = p.base_address?;
                for block in &p.register_blocks {
                    for reg in &block.registers {
                        if reg.name == register_name {
                            return Some(base + reg.offset);
                        }
                    }
                }
            }
        }
        None
    }
}

pub fn lower(prog: &ast::Program) -> Vec<(String, CFG)> {
    let mut signatures = HashMap::new();
    for func in &prog.functions {
        if let Some(sig) = &func.signature {
            signatures.insert(func.name.clone(), sig);
        }
    }
    
    let mut lowered_functions = Vec::new();
    for func in &prog.functions {
        let cfg = lower_function(func, &prog.peripherals, &signatures);
        lowered_functions.push((func.name.clone(), cfg));
    }
    lowered_functions
}

fn lower_function(func: &ast::Function, peripherals: &[ast::Peripheral], signatures: &HashMap<String, &ast::TypeState>) -> CFG {
    let mut ctx = Context::new(peripherals, signatures.clone());

    for (i, (name, _type)) in func.args.iter().enumerate() {
        let reg = ctx.new_register();
        ctx.vars.insert(name.clone(), reg);
        ctx.emit_instr(Instruction::new(
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
            ctx.emit_stmt(Statement::Let {
                var_name: var_name.clone(),
                value: ast_expr_to_cfg(value),
            });
            
            let result_reg = lower_expression(ctx, value);
            ctx.vars.insert(var_name.clone(), result_reg);
        }

        ast::Statement::Assign { var_name, value } => {
            ctx.emit_stmt(Statement::Assign {
                var_name: var_name.clone(),
                value: ast_expr_to_cfg(value),
            });
            
            let value_reg = lower_expression(ctx, value);
            let target_reg = ctx.get_register(var_name);
            
            ctx.emit_instr(Instruction::new(
                Op::Mov,
                Some(target_reg),
                vec![value_reg]
            ));
        }

        ast::Statement::Expr { expr } => {
            if let ast::Expr::FnCall { name, args } = expr {
                if let Some(sig) = ctx.signatures.get(name) {
                    ctx.emit_stmt(Statement::PeripheralDriverCall {
                        function: name.clone(),
                        peripheral: sig.peripheral.clone(),
                        from_state: sig.input_state.clone(),
                        to_state: sig.output_state.clone(),
                    });
                } else {
                    ctx.emit_stmt(Statement::Expr {
                        expr: ast_expr_to_cfg(expr),
                    });
                }
            } else {
                ctx.emit_stmt(Statement::Expr {
                    expr: ast_expr_to_cfg(expr),
                });
            }
            
            lower_expression(ctx, expr);
        }

        ast::Statement::If { cond, then_block, else_block } => {
            let cond_reg = lower_expression(ctx, cond);
            
            let then_bb = ctx.add_block();
            let else_bb = ctx.add_block();
            let merge_bb = ctx.add_block();
            
            // Current block branches based on condition
            ctx.set_terminator(Terminator::Branch {
                cond: cond_reg,
                then_block: then_bb,
                else_block: else_bb,
            });
            
            // Emit then block
            ctx.switch_to(then_bb);
            for s in then_block {
                lower_statement(ctx, s);
            }
            // If then block doesn't have a terminator, jump to merge
            if matches!(ctx.cfg.block(ctx.current_block).terminator, Terminator::None) {
                ctx.set_terminator(Terminator::Jump(merge_bb));
            }
            
            // Emit else block
            ctx.switch_to(else_bb);
            for s in else_block {
                lower_statement(ctx, s);
            }
            // If else block doesn't have a terminator, jump to merge
            if matches!(ctx.cfg.block(ctx.current_block).terminator, Terminator::None) {
                ctx.set_terminator(Terminator::Jump(merge_bb));
            }
            
            // Continue in merge block
            ctx.switch_to(merge_bb);
        }

        ast::Statement::While { cond, body } => {
            let header_bb = ctx.add_block();
            let body_bb = ctx.add_block();
            let exit_bb = ctx.add_block();
            
            // Current block jumps to loop header
            ctx.set_terminator(Terminator::Jump(header_bb));
            
            // Header evaluates condition and branches
            ctx.switch_to(header_bb);
            let cond_reg = lower_expression(ctx, cond);
            ctx.set_terminator(Terminator::Branch {
                cond: cond_reg,
                then_block: body_bb,
                else_block: exit_bb,
            });
            
            // Body executes and loops back to header
            ctx.switch_to(body_bb);
            for s in body {
                lower_statement(ctx, s);
            }
            // If body doesn't have a terminator, jump back to header
            if matches!(ctx.cfg.block(ctx.current_block).terminator, Terminator::None) {
                ctx.set_terminator(Terminator::Jump(header_bb));
            }
            
            // Continue in exit block
            ctx.switch_to(exit_bb);
        }

        ast::Statement::Return { expr } => {
            let value_reg = lower_expression(ctx, expr);
            ctx.set_terminator(Terminator::Return(Some(value_reg)));
        }

        ast::Statement::PeripheralWrite { peripheral, register, value } => {
            ctx.emit_stmt(Statement::PeripheralWrite {
                peripheral: peripheral.clone(),
                register: register.clone(),
                value: ast_expr_to_cfg(value),
            });
            
            let value_reg = lower_expression(ctx, value);
            
            let addr = ctx.get_mmio_address(peripheral, register)
                .expect(&format!("Unknown peripheral register {}.{}", peripheral, register));
            
            let addr_reg = ctx.new_register();
            ctx.emit_instr(Instruction::new(
                Op::LoadAddr(addr),
                Some(addr_reg),
                vec![]
            ));
            
            ctx.emit_instr(Instruction::new(
                Op::StoreWord,
                None,
                vec![value_reg, addr_reg]
            ));
        }
    }
}

fn lower_expression(ctx: &mut Context, expr: &ast::Expr) -> VirtualRegister {
    match expr {
        ast::Expr::IntLit { value } => {
            let dest = ctx.new_register();
            ctx.emit_instr(Instruction::new(
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
            ctx.emit_instr(Instruction::new(
                Op::Call(name.clone()),
                Some(dest),
                arg_regs
            ));
            dest
        }

        ast::Expr::PeripheralRead { peripheral, register } => {
            let addr = ctx.get_mmio_address(peripheral, register)
                .expect(&format!("Unknown peripheral register {}.{}", peripheral, register));
            
            let addr_reg = ctx.new_register();
            ctx.emit_instr(Instruction::new(
                Op::LoadAddr(addr),
                Some(addr_reg),
                vec![]
            ));
            
            let dest = ctx.new_register();
            ctx.emit_instr(Instruction::new(
                Op::LoadWord,
                Some(dest),
                vec![addr_reg]
            ));
            dest
        }
        
        ast::Expr::Binary { op, left, right } => {
            let left_reg = lower_expression(ctx, left);
            let right_reg = lower_expression(ctx, right);
            
            let ir_op = match op {
                ast::BinaryOp::Add => Op::Add,
                ast::BinaryOp::Sub => Op::Sub,
                ast::BinaryOp::Mul => Op::Mul,
                ast::BinaryOp::Div => Op::Div,
                ast::BinaryOp::Mod => Op::Rem,
                ast::BinaryOp::BitAnd => Op::And,
                ast::BinaryOp::BitOr => Op::Or,
                ast::BinaryOp::BitXor => Op::Xor,
                ast::BinaryOp::Shl => Op::Sll,
                ast::BinaryOp::Shr => Op::Srl,
                ast::BinaryOp::Eq => Op::Eq,
                ast::BinaryOp::Ne => Op::Ne,
                ast::BinaryOp::Lt => Op::Lt,
                ast::BinaryOp::Le => Op::Le,
                ast::BinaryOp::Gt => Op::Gt,
                ast::BinaryOp::Ge => Op::Ge,
                // TODO: Implement && and || short circuiting
                ast::BinaryOp::And => Op::And,
                ast::BinaryOp::Or => Op::Or,
            };
            
            let dest = ctx.new_register();
            ctx.emit_instr(Instruction::new(
                ir_op,
                Some(dest),
                vec![left_reg, right_reg]
            ));
            dest
        }
        
        ast::Expr::Unary { op, operand } => {
            let operand_reg = lower_expression(ctx, operand);
            
            let ir_op = match op {
                ast::UnaryOp::Neg => Op::Neg,
                ast::UnaryOp::Not => Op::Not,
                ast::UnaryOp::BitNot => Op::Not,
            };
            
            let dest = ctx.new_register();
            ctx.emit_instr(Instruction::new(
                ir_op,
                Some(dest),
                vec![operand_reg]
            ));
            dest
        }
    }
}

// Convert AST expression to CFG expression
fn ast_expr_to_cfg(expr: &ast::Expr) -> Expr {
    match expr {
        ast::Expr::IntLit { value } => Expr::IntLit { value: *value },
        ast::Expr::Variable { name } => Expr::Variable { name: name.clone() },
        ast::Expr::Binary { op, left, right } => Expr::Binary {
            op: *op,
            left: Box::new(ast_expr_to_cfg(left)),
            right: Box::new(ast_expr_to_cfg(right)),
        },
        ast::Expr::Unary { op, operand } => Expr::Unary {
            op: *op,
            operand: Box::new(ast_expr_to_cfg(operand)),
        },
        ast::Expr::PeripheralRead { peripheral, register } => Expr::PeripheralRead {
            peripheral: peripheral.clone(),
            register: register.clone(),
        },
        ast::Expr::FnCall { name, args } => Expr::FnCall {
            name: name.clone(),
            args: args.iter().map(ast_expr_to_cfg).collect(),
        },
    }
}