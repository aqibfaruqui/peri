pub mod lower;
pub mod cfg;

pub use cfg::CmpOp;
#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub struct VirtualRegister {
    pub id: usize,
}

#[derive(Debug, Clone)]
pub struct Instruction {
    pub operation: Op,
    pub destination: Option<VirtualRegister>,
    pub args: Vec<VirtualRegister>,
}

// TODO: Extend with more instructions
#[derive(Debug, Clone)]
pub enum Op {
    LoadImm(i32),                   // li t0, 5
    LoadAddr(u32),                  // li t0, 0x40000000
    LoadWord,                       // lw t1, 0(t0)
    StoreWord,                      // sw t0, 0(t1)
    Mov,                            // mv t1, t0
    MovArg(usize),                  // mv a0, t1
    Call(String),                   // call func
    Ret(Option<VirtualRegister>),   // [mv a0, t1] ret
    Label(String),                  // .LBB_func_0
    Jump(String),                   // j .LBB_func_0
    BranchIfFalse(String),          // beqz t0, .LBB_func_end
    BranchCond(CmpOp, String),      // beq/bne/blt/bge rs1, rs2, label
    Add,                            // add rd, rs1, rs2
    Sub,                            // sub rd, rs1, rs2
    Mul,                            // mul rd, rs1, rs2
    Div,                            // div rd, rs1, rs2
    Rem,                            // rem rd, rs1, rs2 (modulo)
    And,                            // and rd, rs1, rs2
    Or,                             // or rd, rs1, rs2
    Xor,                            // xor rd, rs1, rs2
    Sll,                            // sll rd, rs1, rs2 (shift left logical)
    Srl,                            // srl rd, rs1, rs2 (shift right logical)
    Neg,                            // neg rd, rs (sub rd, x0, rs)
    Not,                            // not rd, rs (xori rd, rs, -1)
}

impl Instruction {
    pub fn new(
        operation: Op, 
        destination: Option<VirtualRegister>, 
        args: Vec<VirtualRegister>
    ) -> Self {
        Self { operation, destination, args }
    }
}