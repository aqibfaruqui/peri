#[derive(Debug)]
pub struct Program {
    pub functions: Vec<Function>,
}

#[derive(Debug)]
pub struct Function {
    pub name: String,
    pub args: Vec<String>,
    pub return_type: String,    // change to enum of possible types later?
    pub body: Vec<Statement>,
}

#[derive(Debug)]
pub enum Statement {
    // `let timer_a = get_timer();`
    Let { var_name: String, value: Expr },
    // `enable(timer_a);`
    Expr { expr: Expr },
}

#[derive(Debug)]
pub enum Expr {
    // `get_timer()` or `enable(timer_a)`
    FnCall { name: String, args: Vec<Expr> },
    // `timer_a`
    Variable { name: String },
}