pub mod regalloc;
pub mod generator;

use crate::frontend::ast;
use crate::ir;

pub fn compile(prog: &ast::Program) -> Result<String, String> {
    let functions = ir::lower::lower(prog);
    let mut assembly = String::new();

    for (func_name, cfg) in functions {
        // TODO: Make regalloc and generator aware of CFG (currently assumes linear instructions)
        let instructions = cfg.flatten();
        
        let allocation = regalloc::allocate(&instructions);
        let asm = generator::generate(&func_name, &instructions, &allocation)
            .map_err(|e| e.to_string())?;

        assembly.push_str(&asm);
    }

    Ok(assembly)
}