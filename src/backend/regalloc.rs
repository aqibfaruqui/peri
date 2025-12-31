use std::collections::HashMap;
use crate::ir::{VirtualRegister, Instruction};

pub type Allocation = HashMap<VirtualRegister, String>;

pub const REGISTERS: [&str; 7] = ["t0", "t1", "t2", "t3", "t4", "t5", "t6"];

pub fn allocate(instructions: &Vec<Instruction>) -> Allocation {
    let mut map = HashMap::new();
    
    // TODO: Implement live intervals for linear scan
    for instr in instructions {
        if let Some(dest) = instr.destination {
            if !map.contains_key(&dest) {
                // TODO: Remove 'Mod 7' allocator (used for basic testing)
                let reg = REGISTERS[dest.id % REGISTERS.len()];
                map.insert(dest, reg.to_string());
            }
        }
        
        // TODO: Don't map arguments to t_ registers
        for arg in &instr.args {
            if !map.contains_key(arg) {
                 let reg = REGISTERS[arg.id % REGISTERS.len()];
                 map.insert(*arg, reg.to_string());
            }
        }
    }
    map
}