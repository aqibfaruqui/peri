#[derive(Debug, Clone)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub args: Vec<String>,
    pub signature: TypeState,
    pub body: Vec<Statement>,
}

#[derive(Debug, Clone)]
pub struct TypeState {
    pub peripheral: String,
    pub input_state: String,
    pub output_state: String,
}

#[derive(Debug, Clone)]
pub enum Statement {
    Let { var_name: String, value: Expr },
    Assign { var_name: String, value: Expr },
    Expr { expr: Expr },
}

#[derive(Debug, Clone)]
pub enum Expr {
    IntLit {value: i32},            // TODO: Test if better as IntLit(i32) and Variable(String)
    FnCall { name: String, args: Vec<Expr> },
    Variable { name: String },
}
