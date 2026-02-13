pub mod lower;
pub mod cfg;

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
    Label(String),                  // .X_loop_start
    Jump(String),                   // j .X_loop_start
    BranchIfFalse(String),          // beqz t0, .L_end
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
    Eq,                             // seq: set if equal
    Ne,                             // sne: set if not equal
    Lt,                             // slt rd, rs1, rs2
    Le,                             // sle: set if less or equal
    Gt,                             // sgt: set if greater
    Ge,                             // sge: set if greater or equal
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