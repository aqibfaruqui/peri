pub mod regalloc;
pub mod generator;
pub mod liveness;

use crate::frontend::ast;
use crate::frontend::verifier;
use crate::ir;

pub fn compile(prog: &ast::Program) -> Result<String, String> {
    let signatures = verifier::build_signature_map(prog);
    
    let functions = ir::lower::lower(prog);
    
    // TODO: Move verification calls to main?
    for (i, (func_name, cfg)) in functions.iter().enumerate() {
        let func = &prog.functions[i];
        if let Err(err) = verifier::verify_function(func, cfg, &prog.peripherals, &signatures) {
            return Err(format!("Typestate error in function '{}': {:?}", func_name, err));
        }
    }
    
    let mut assembly = String::new();

    for (func_name, cfg) in functions {
        let allocation = regalloc::allocate(&cfg);
        
        let instructions = cfg.flatten();
        let asm = generator::generate(&func_name, &instructions, &allocation)
            .map_err(|e| e.to_string())?;

        assembly.push_str(&asm);
    }

    Ok(assembly)
}