pub mod regalloc;
pub mod generator;
pub mod liveness;

use crate::ir;

pub fn generate(functions: &[(String, ir::cfg::CFG)]) -> Result<String, String> {
    let mut assembly = String::new();

    for (function, cfg) in functions {
        let allocation = regalloc::allocate(cfg);
        
        let instructions = cfg.flatten();
        let asm = generator::generate(function, &instructions, &allocation)
            .map_err(|e| e.to_string())?;

        assembly.push_str(&asm);
    }

    Ok(assembly)
}