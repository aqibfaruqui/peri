use std::collections::{HashMap, HashSet};
use crate::ir::{VirtualRegister, Op};
use crate::ir::cfg::{CFG, Terminator};
use crate::backend::liveness;

pub type Allocation = HashMap<VirtualRegister, String>;

/* RV32I Register Classes */

#[allow(dead_code)] /* Currently hardcoded in generator.rs, TODO: Properly handle >8 arguments */
pub const A_REGS: [&str; 8] = ["a0", "a1", "a2", "a3", "a4", "a5", "a6", "a7"];

pub const T_REGS: [&str; 7] = ["t0", "t1", "t2", "t3", "t4", "t5", "t6"];

pub const S_REGS: [&str; 11] = ["s1", "s2", "s3", "s4", "s5", "s6",
                                  "s7", "s8", "s9", "s10", "s11"];  // TODO: Check if s0/fp can be used

#[derive(Debug, Clone)]
pub struct LiveInterval {
    pub vreg: VirtualRegister,
    pub start: usize,
    pub end: usize,
    pub crosses_call: bool,         /* Interval crossing 'call' instruction must be moved to S_REG */
}

pub struct AllocationResult {
    pub allocation: Allocation,
    pub used_s_regs: Vec<String>,   /* Which S_REGs this function uses */
}

pub fn allocate(cfg: &CFG) -> AllocationResult {
    let liveness = liveness::analyse(cfg);
    let (intervals, call_points) = build_intervals(cfg, &liveness);
    linear_scan(intervals, &call_points)
}

fn build_intervals(
    cfg: &CFG,
    liveness: &liveness::LivenessResult,
) -> (Vec<LiveInterval>, HashSet<usize>) {
    let mut intervals: HashMap<VirtualRegister, (usize, usize)> = HashMap::new();
    let mut call_points: HashSet<usize> = HashSet::new();
    let mut program_point = 0;

    for block in &cfg.blocks {
        if let Some(b) = liveness.get(&block.id) {
            for vreg in &b.live_in {
                extend(&mut intervals, *vreg, program_point, program_point);
            }
        }

        for instr in &block.instructions {
            if matches!(&instr.operation, Op::Call(_)) {
                call_points.insert(program_point);
            }
            if let Some(dest) = instr.destination {
                extend(&mut intervals, dest, program_point, program_point);
            }
            for arg in &instr.args {
                extend(&mut intervals, *arg, program_point, program_point);
            }
            program_point += 1;
        }

        match &block.terminator {
            Terminator::Branch { cond, .. } => {
                extend(&mut intervals, *cond, program_point, program_point);
            }
            Terminator::Return(Some(reg)) => {
                extend(&mut intervals, *reg, program_point, program_point);
            }
            _ => {}
        }

        if let Some(b) = liveness.get(&block.id) {
            for vreg in &b.live_out {
                extend(&mut intervals, *vreg, program_point, program_point);
            }
        }
    }

    let mut result: Vec<LiveInterval> = intervals
        .into_iter()
        .map(|(vreg, (start, end))| {
            let crosses_call = call_points.iter().any(|&p| p >= start && p < end);
            LiveInterval { vreg, start, end, crosses_call }
        })
        .collect();

    result.sort_by_key(|i| i.start);
    (result, call_points)
}

fn extend(
    intervals: &mut HashMap<VirtualRegister, (usize, usize)>,
    vreg: VirtualRegister,
    point: usize,
    _end: usize,
) {
    intervals
        .entry(vreg)
        .and_modify(|(start, end)| {
            *start = (*start).min(point);
            *end = (*end).max(point);
        })
        .or_insert((point, point));
}

fn linear_scan(intervals: Vec<LiveInterval>, _call_points: &HashSet<usize>) -> AllocationResult {
    let mut allocation = Allocation::new();

    let mut free_t: Vec<String> = T_REGS.iter().rev().map(|s| s.to_string()).collect();
    let mut free_s: Vec<String> = S_REGS.iter().rev().map(|s| s.to_string()).collect();
    let mut active_t: Vec<(LiveInterval, String)> = Vec::new();
    let mut active_s: Vec<(LiveInterval, String)> = Vec::new();
    let mut used_s: HashSet<String> = HashSet::new();

    for interval in intervals {
        expire(&mut active_t, &mut free_t, interval.start);
        expire(&mut active_s, &mut free_s, interval.start);

        if interval.crosses_call { /* Allocate to S_REG */
            allocate_from(
                interval,
                &mut free_s,
                &mut active_s,
                &mut allocation,
                &mut used_s,
                S_REGS.len(),
                /* fallback_pool */ S_REGS.as_slice(),
                /* is_s */ true,
            );

        } else { /* Allocate to T_REG */
            if let Some(reg) = free_t.pop() {
                allocation.insert(interval.vreg, reg.clone());
                active_t.push((interval, reg));

            } else if let Some(reg) = free_s.pop() { /* T_REGs exhausted, spill into S_REG. */
                used_s.insert(reg.clone());
                allocation.insert(interval.vreg, reg.clone());
                active_s.push((interval, reg));

            } else { /* All T_REGs and S_REGs exhausted, TODO: implement proper stack spilling */
                eprintln!("WARNING: register count exceeded, using modulo fallback");
                let id = interval.vreg.id;
                let reg = T_REGS[id % T_REGS.len()].to_string();
                allocation.insert(interval.vreg, reg);
            }
        }
    }

    let mut used_s_regs: Vec<String> = used_s.into_iter().collect::<Vec<_>>();
    used_s_regs.sort();

    AllocationResult { allocation, used_s_regs }
}

fn allocate_from(
    interval: LiveInterval,
    free: &mut Vec<String>,
    active: &mut Vec<(LiveInterval, String)>,
    allocation: &mut Allocation,
    used_s: &mut HashSet<String>,
    pool_len: usize,
    fallback: &[&str],
    is_s: bool,
) {
    if let Some(reg) = free.pop() {
        if is_s { used_s.insert(reg.clone()); }
        allocation.insert(interval.vreg, reg.clone());
        active.push((interval, reg));
    } else {
        eprintln!("WARNING: out of {:?} registers, using modulo fallback", fallback);
        let reg = fallback[interval.vreg.id % pool_len].to_string();
        if is_s { used_s.insert(reg.clone()); }
        allocation.insert(interval.vreg, reg);
    }
}

fn expire(
    active: &mut Vec<(LiveInterval, String)>,
    free: &mut Vec<String>,
    current_start: usize,
) {
    let mut freed = Vec::new();
    active.retain(|(interval, reg)| {
        if interval.end < current_start {
            freed.push(reg.clone());
            false
        } else {
            true
        }
    });
    free.extend(freed);
}