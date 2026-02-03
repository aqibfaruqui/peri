use std::collections::{HashMap, HashSet};
use crate::ir::VirtualRegister;
use crate::ir::cfg::{CFG, BlockId, Terminator};

#[derive(Debug, Clone, Default)]
pub struct BlockLiveness {
    pub live_in: HashSet<VirtualRegister>,      // Variables live at the start of this block
    pub live_out: HashSet<VirtualRegister>,     // Variables live at the end of this block
    pub use_set: HashSet<VirtualRegister>,      // Variables used before being defined in this block
    pub def_set: HashSet<VirtualRegister>,      // Variables defined in this block
}

pub type LivenessResult = HashMap<BlockId, BlockLiveness>;

pub fn analyse(cfg: &CFG) -> LivenessResult {
    let mut result: LivenessResult = HashMap::new();
    
    for block in &cfg.blocks {
        let mut block_liveness = BlockLiveness::default();
        
        for instr in &block.instructions {
            for arg in &instr.args {
                if !block_liveness.def_set.contains(arg) {
                    block_liveness.use_set.insert(*arg);
                }
            }
            
            if let Some(dest) = instr.destination {
                block_liveness.def_set.insert(dest);
            }
        }
        
        match &block.terminator {
            Terminator::Branch { cond, .. } => {
                if !block_liveness.def_set.contains(cond) {
                    block_liveness.use_set.insert(*cond);
                }
            }
            Terminator::Return(Some(reg)) => {
                if !block_liveness.def_set.contains(reg) {
                    block_liveness.use_set.insert(*reg);
                }
            }
            _ => {}
        }
        
        result.insert(block.id, block_liveness);
    }
    
    let mut changed = true;
    while changed {
        changed = false;
        
        for block in cfg.blocks.iter().rev() {
            let successors = get_successors(&block.terminator);
            
            let mut new_live_out: HashSet<VirtualRegister> = HashSet::new();
            for succ_id in &successors {
                if let Some(succ_liveness) = result.get(succ_id) {
                    new_live_out.extend(&succ_liveness.live_in);
                }
            }
            
            let block_liveness = result.get(&block.id).unwrap();
            let live_out_minus_def: HashSet<_> = new_live_out
                .difference(&block_liveness.def_set)
                .cloned()
                .collect();
            let new_live_in: HashSet<_> = block_liveness.use_set
                .union(&live_out_minus_def)
                .cloned()
                .collect();
            
            let block_liveness = result.get_mut(&block.id).unwrap();
            if new_live_in != block_liveness.live_in || new_live_out != block_liveness.live_out {
                changed = true;
                block_liveness.live_in = new_live_in;
                block_liveness.live_out = new_live_out;
            }
        }
    }
    
    result
}

fn get_successors(term: &Terminator) -> Vec<BlockId> {
    match term {
        Terminator::Jump(target) => vec![*target],
        Terminator::Branch { then_block, else_block, .. } => vec![*then_block, *else_block],
        Terminator::Fallthrough(target) => vec![*target],
        Terminator::Return(_) => vec![],
        Terminator::None => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Instruction, Op};
    use crate::ir::cfg::BasicBlock;

    #[test]
    fn test_simple_liveness() {
        // BB0: x = 10; return x
        let mut cfg = CFG::new();
        let bb0 = cfg.add_block();
        
        let x = VirtualRegister { id: 0 };
        cfg.block_mut(bb0).push(Instruction::new(Op::LoadImm(10), Some(x), vec![]));
        cfg.block_mut(bb0).set_terminator(Terminator::Return(Some(x)));
        
        let result = analyse(&cfg);
        let bb0_liveness = result.get(&bb0).unwrap();
        
        // x is defined and used in same block, but used in terminator
        assert!(bb0_liveness.def_set.contains(&x));
        assert!(bb0_liveness.use_set.contains(&x)); // Used in return before any def visible
        assert!(bb0_liveness.live_in.contains(&x)); // Hmm, this might be wrong...
    }
}
