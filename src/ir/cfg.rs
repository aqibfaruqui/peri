use crate::ir::{Instruction, Op, VirtualRegister};

pub type BlockId = usize;

#[derive(Debug, Clone)]
pub struct CFG {
    pub blocks: Vec<BasicBlock>,
    pub entry: BlockId,
}

impl CFG {
    pub fn new() -> Self {
        Self {
            blocks: Vec::new(),
            entry: 0,
        }
    }

    pub fn add_block(&mut self) -> BlockId {
        let id = self.blocks.len();
        self.blocks.push(BasicBlock::new(id));
        id
    }

    pub fn block_mut(&mut self, id: BlockId) -> &mut BasicBlock {
        &mut self.blocks[id]
    }

    pub fn block(&self, id: BlockId) -> &BasicBlock {
        &self.blocks[id]
    }

    /*
     * TODO: Fix temporary fix below
     * 
     * Flatten CFG back to linear instruction stream (backend currently uses this)
     * - For each block (except entry which uses function name), give a label
     * - Emit all instructions in the block
     * - Convert the terminator to instruction(s)
     */
    pub fn flatten(&self, func_name: &str) -> Vec<Instruction> {
        let mut instructions = Vec::new();
        let label = |id: usize| format!(".LBB_{}_{}", func_name, id);

        for block in &self.blocks {
            if block.id != self.entry {
                instructions.push(Instruction::new(
                    Op::Label(label(block.id)),
                    None,
                    vec![],
                ));
            }

            instructions.extend(block.instructions.clone());

            match &block.terminator {
                Terminator::Jump(target) => {
                    instructions.push(Instruction::new(
                        Op::Jump(label(*target)),
                        None,
                        vec![],
                    ));
                }

                Terminator::Branch { cond, then_block, else_block } => {
                    instructions.push(Instruction::new(
                        Op::BranchIfFalse(label(*else_block)),
                        None,
                        vec![*cond],
                    ));

                    if *then_block != block.id + 1 {
                        instructions.push(Instruction::new(
                            Op::Jump(label(*then_block)),
                            None,
                            vec![],
                        ));
                    }
                }

                Terminator::CondBranch { op, lhs, rhs, then_block, else_block } => {
                    instructions.push(Instruction::new(
                        Op::BranchCond(*op, label(*else_block)),
                        None,
                        vec![*lhs, *rhs],
                    ));

                    if *then_block != block.id + 1 {
                        instructions.push(Instruction::new(
                            Op::Jump(label(*then_block)),
                            None,
                            vec![],
                        ));
                    }
                }

                Terminator::Return(val) => {
                    instructions.push(Instruction::new(
                        Op::Ret(*val),
                        None,
                        val.map_or(vec![], |v| vec![v]),
                    ));
                }

                Terminator::None => {
                    // No terminator shouldn't happen in a well formed CFG :)
                }
            }
        }
        
        instructions
    }
}

#[derive(Debug, Clone)]
pub struct BasicBlock {
    pub id: BlockId,
    pub statements: Vec<Statement>,         // For verification
    pub instructions: Vec<Instruction>,     // For codegen
    pub terminator: Terminator,
}

impl BasicBlock {
    pub fn new(id: BlockId) -> Self {
        Self {
            id,
            statements: Vec::new(),
            instructions: Vec::new(),
            terminator: Terminator::None,
        }
    }
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Statement {
    PeripheralDriverCall {
        function: String,
        peripheral: String,
        from_state: String,
        to_state: String,
    },
    
    Let {
        var_name: String,
        value: Expr,
    },
    
    Assign {
        var_name: String,
        value: Expr,
    },
    
    PeripheralWrite {
        peripheral: String,
        register: String,
        value: Expr,
    },
    
    Expr {
        expr: Expr,
    },
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
pub enum Expr {
    IntLit { value: i32 },
    Variable { name: String },
    
    Binary {
        op: crate::frontend::ast::BinaryOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    
    Unary {
        op: crate::frontend::ast::UnaryOp,
        operand: Box<Expr>,
    },
    
    PeripheralRead {
        peripheral: String,
        register: String,
    },
    
    FnCall {
        name: String,
        args: Vec<Expr>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CmpOp { Eq, Ne, Lt, Le, Gt, Ge }

#[derive(Debug, Clone)]
pub enum Terminator {
    Jump(BlockId),          // Unconditional jump to another block
    Branch {                // Conditional jump to another block (based on cond)
        cond: VirtualRegister,
        then_block: BlockId,
        else_block: BlockId,
    },
    CondBranch {            // Conditional jump on a comparison of two registers
        op: CmpOp,
        lhs: VirtualRegister,
        rhs: VirtualRegister,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return(Option<VirtualRegister>),    // Return from the function, optionally with a value
    None,
}
