use std::collections::HashMap;
use crate::ir::VirtualRegister;
use crate::ir::cfg::{CFG, BlockId};
use crate::backend::liveness;

pub type Allocation = HashMap<VirtualRegister, String>;

pub const REGISTERS: [&str; 7] = ["t0", "t1", "t2", "t3", "t4", "t5", "t6"];

#[derive(Debug, Clone)]
pub struct LiveInterval {
    pub vreg: VirtualRegister,
    pub start: usize,
    pub end: usize,
}

pub fn allocate(cfg: &CFG) -> Allocation {
    let liveness = liveness::analyse(cfg);              // Step 1: Compute liveness information
    let intervals = build_intervals(cfg, &liveness);    // Step 2: Number instructions and build intervals
    linear_scan(intervals)                              // Step 3: Linear scan register allocation
}

fn build_intervals(cfg: &CFG, liveness: &liveness::LivenessResult) -> Vec<LiveInterval> {
    let mut intervals: HashMap<VirtualRegister, (usize, usize)> = HashMap::new();
    let mut program_point = 0;
    
    for block in &cfg.blocks {
        if let Some(block_liveness) = liveness.get(&block.id) {
            for vreg in &block_liveness.live_in {
                intervals.entry(*vreg)
                    .and_modify(|(start, end)| {
                        *start = (*start).min(program_point);
                        *end = (*end).max(program_point);
                    })
                    .or_insert((program_point, program_point));
            }
        }
        
        for instr in &block.instructions {
            if let Some(dest) = instr.destination {
                intervals.entry(dest)
                    .and_modify(|(_, end)| *end = (*end).max(program_point))
                    .or_insert((program_point, program_point));
            }
            
        for arg in &instr.args {
                intervals.entry(*arg)
                    .and_modify(|(_, end)| *end = (*end).max(program_point))
                    .or_insert((program_point, program_point));
            }
            
            program_point += 1;
        }
        
        match &block.terminator {
            crate::ir::cfg::Terminator::Branch { cond, .. } => {
                intervals.entry(*cond)
                    .and_modify(|(_, end)| *end = (*end).max(program_point))
                    .or_insert((program_point, program_point));
            }
            crate::ir::cfg::Terminator::Return(Some(reg)) => {
                intervals.entry(*reg)
                    .and_modify(|(_, end)| *end = (*end).max(program_point))
                    .or_insert((program_point, program_point));
            }
            _ => {}
        }
        
        // Variables live out of this block need their interval extended
        if let Some(block_liveness) = liveness.get(&block.id) {
            for vreg in &block_liveness.live_out {
                intervals.entry(*vreg)
                    .and_modify(|(_, end)| *end = (*end).max(program_point))
                    .or_insert((program_point, program_point));
            }
        }
    }
    
    let mut result: Vec<LiveInterval> = intervals
        .into_iter()
        .map(|(vreg, (start, end))| LiveInterval { vreg, start, end })
        .collect();
    
    // Sort by start point for linear scan
    result.sort_by_key(|i| i.start);
    result
}

fn linear_scan(intervals: Vec<LiveInterval>) -> Allocation {
    let mut allocation = Allocation::new();
    let mut active: Vec<(LiveInterval, String)> = Vec::new();
    let mut free: Vec<String> = REGISTERS.iter().rev().map(|s| s.to_string()).collect();
    
    for interval in intervals {
        // Expire old intervals whose end < current start
        let mut expired_regs: Vec<String> = Vec::new();
        active.retain(|(old_interval, reg)| {
            if old_interval.end < interval.start {
                expired_regs.push(reg.clone());
                false
            } else {
                true
            }
        });
        free.extend(expired_regs);
        
        // Sort active by end point for potential spilling
        active.sort_by_key(|(i, _)| i.end);
        
        if let Some(reg) = free.pop() {
            allocation.insert(interval.vreg, reg.clone());
            active.push((interval, reg));
        } else if !active.is_empty() {
            let last_idx = active.len() - 1;
            let (spill_candidate, spill_reg) = active[last_idx].clone();
            
            if spill_candidate.end > interval.end {
                allocation.insert(interval.vreg, spill_reg.clone());
                active.remove(last_idx);
                active.push((interval, spill_reg));
                // TODO: Mark spill_candidate for stack slot and implement spilling
                eprintln!("WARNING: Would spill {:?} but spilling not implemented", spill_candidate.vreg);
            } else {
                eprintln!("WARNING: Would spill {:?} but spilling not implemented", interval.vreg);
                // TODO: Remove fallback of modulo allocation
                let reg = REGISTERS[interval.vreg.id % REGISTERS.len()];
                allocation.insert(interval.vreg, reg.to_string());
            }
        } else {
            // TODO: Check if no registers case needs this
            let reg = REGISTERS[interval.vreg.id % REGISTERS.len()];
            allocation.insert(interval.vreg, reg.to_string());
        }
    }

    allocation
}