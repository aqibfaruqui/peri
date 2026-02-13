#[derive(Debug, Clone)]
pub struct Program {
    pub peripherals: Vec<Peripheral>,
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Peripheral {
    pub name: String,
    pub base_address: Option<u32>,
    pub states: Vec<String>,
    pub initial: String,
    pub register_blocks: Vec<RegisterBlock>,
}

#[derive(Debug, Clone)]
pub struct RegisterBlock {
    pub reg_type: RegisterType,
    pub registers: Vec<Register>,
}

#[derive(Debug, Clone)]
pub struct Register {
    pub name: String,
    pub offset: u32,
}

#[derive(Debug, Clone)]
pub enum RegisterType {
    U8,
    U16,
    U32,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub args: Vec<(String, Type)>,
    pub signature: Option<TypeState>,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct TypeState {
    pub peripheral: String,
    pub input_state: String,
    pub output_state: String,
}

#[derive(Debug, Clone)]
pub enum Type {
    I32,
    U8,
    U16,
    U32,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Let { var_name: String, value: Expr },
    Assign { var_name: String, value: Expr },
    Expr { expr: Expr },
    If { cond: Expr, then_block: Vec<Statement>, else_block: Vec<Statement>},
    While { cond: Expr, body: Vec<Statement>},
    Return { expr: Expr },
    PeripheralWrite { peripheral: String, register: String, value: Expr },
}

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit { value: i32 },
    Variable { name: String },
    Binary { op: BinaryOp, left: Box<Expr>, right: Box<Expr> },
    Unary { op: UnaryOp, operand: Box<Expr> },
    FnCall { name: String, args: Vec<Expr> },
    PeripheralRead { peripheral: String, register: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,    
    Sub,    
    Mul,    
    Div,    
    Mod,    
    BitAnd, 
    BitOr,  
    BitXor, 
    Shl,    
    Shr,    
    Eq,     
    Ne,     
    Lt,     
    Le,     
    Gt,     
    Ge,     
    And,    
    Or,     
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,    
    Not,    
    BitNot, 
}
