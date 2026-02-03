pub mod lower;

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
    Mov,                            // mv t1, t0
    MovArg(usize),                  // mv a0, t1
    Call(String),                   // call func
    Ret(Option<VirtualRegister>),   // [mv a0, t1] ret
    Label(String),                  // .X_loop_start
    Jump(String),                   // j .X_loop_start
    BranchIfFalse(String),          // beqz t0, .L_end
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