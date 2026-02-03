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
    pub fn flatten(&self) -> Vec<Instruction> {
        let mut instructions = Vec::new();
        
        for block in &self.blocks {
            if block.id != self.entry {
                instructions.push(Instruction::new(
                    Op::Label(format!(".LBB{}", block.id)),
                    None,
                    vec![],
                ));
            }

            instructions.extend(block.instructions.clone());

            match &block.terminator {
                Terminator::Jump(target) => {
                    instructions.push(Instruction::new(
                        Op::Jump(format!(".LBB{}", target)),
                        None,
                        vec![],
                    ));
                }

                Terminator::Branch { cond, then_block, else_block } => {
                    instructions.push(Instruction::new(
                        Op::BranchIfFalse(format!(".LBB{}", else_block)),
                        None,
                        vec![*cond],
                    ));

                    if *then_block != block.id + 1 {
                        instructions.push(Instruction::new(
                            Op::Jump(format!(".LBB{}", then_block)),
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

                Terminator::Fallthrough(target) => {
                    if *target != block.id + 1 {
                        instructions.push(Instruction::new(
                            Op::Jump(format!(".LBB{}", target)),
                            None,
                            vec![],
                        ));
                    }
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
    pub instructions: Vec<Instruction>,
    pub terminator: Terminator,
}

impl BasicBlock {
    pub fn new(id: BlockId) -> Self {
        Self {
            id,
            instructions: Vec::new(),
            terminator: Terminator::None,
        }
    }

    pub fn push(&mut self, instr: Instruction) {
        self.instructions.push(instr);
    }

    pub fn set_terminator(&mut self, term: Terminator) {
        self.terminator = term;
    }
}

#[derive(Debug, Clone)]
pub enum Terminator {
    Jump(BlockId),          // Unconditional jump to another block
    Branch {                // Conditional jump to another block
        cond: VirtualRegister,
        then_block: BlockId,
        else_block: BlockId,
    },
    Return(Option<VirtualRegister>),    // Return from the function, optionally with a value
    Fallthrough(BlockId),               // Fallthrough to the next block (implicit jump)
    None,
}
