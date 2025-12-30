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
    LoadImm(i32),
    Mov,
    Call(String),
    Ret,
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