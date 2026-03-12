use crate::ir::{Instruction, Op};
use crate::ir::cfg::CmpOp;
use crate::backend::regalloc::AllocationResult;
use std::fmt::Write;

/* RV32I Stack frame layout (grows downward, 16-byte aligned)

  sp + frame_size - 4  <- ra
  sp + frame_size - 8  <- s_regs[0]  (first used callee-saved reg)
  sp + frame_size - 12 <- s_regs[1]
  ...
  sp + 0

frame_size = round_up_16(4 + 4 * num_s_regs) */

pub fn generate(
    function: &str,
    instructions: &[Instruction],
    result: &AllocationResult,
) -> Result<String, std::fmt::Error> {
    let allocation = &result.allocation;
    let s_regs = &result.used_s_regs;
    let raw = 4 + 4 * s_regs.len();
    let frame_size = (raw + 15) & !15;
    let ra_offset = frame_size - 4;

    let mut output = String::new();

    writeln!(output, ".section .text")?;
    writeln!(output, ".global {}", function)?;
    writeln!(output, "{}:", function)?;
    writeln!(output, "    addi sp, sp, -{}", frame_size)?;
    writeln!(output, "    sw ra, {}(sp)", ra_offset)?;

    for (i, reg) in s_regs.iter().enumerate() {
        let offset = ra_offset as isize - 4 - (i as isize * 4);
        writeln!(output, "    sw {}, {}(sp)", reg, offset)?;
    }

    for instr in instructions {
        match &instr.operation {
            Op::LoadImm(val) => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                writeln!(output, "    li {}, {}", rd, val)?;
            }

            Op::LoadAddr(addr) => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                writeln!(output, "    li {}, 0x{:08x}", rd, addr)?;
            }

            Op::LoadWord => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs = allocation.get(&instr.args[0]).unwrap();
                writeln!(output, "    lw {}, 0({})", rd, rs)?;
            }

            Op::StoreWord => {
                let rs = allocation.get(&instr.args[0]).unwrap();
                let rd = allocation.get(&instr.args[1]).unwrap();
                writeln!(output, "    sw {}, 0({})", rs, rd)?;
            }

            Op::Mov => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs = allocation.get(&instr.args[0]).unwrap();
                writeln!(output, "    mv {}, {}", rd, rs)?;
            }

            Op::MovArg(i) => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                // TODO: Panic / Error if i >= 8 (we only have a0...a7)
                writeln!(output, "    mv {}, a{}", rd, i)?;
            }

            Op::Call(target) => {
                for (i, arg) in instr.args.iter().enumerate() {
                    let rs = allocation.get(arg).unwrap();
                    writeln!(output, "    mv a{}, {}", i, rs)?;
                }

                writeln!(output, "    call {}", target)?;

                if let Some(dest) = instr.destination {
                    let rd = allocation.get(&dest).unwrap();
                    writeln!(output, "    mv {}, a0", rd)?;
                }
            }

            Op::Ret(val) => {
                if let Some(reg) = val {
                    let rs = allocation.get(reg).unwrap();
                    writeln!(output, "    mv a0, {}", rs)?;
                }

                for (i, reg) in s_regs.iter().enumerate() {
                    let offset = ra_offset as isize - 4 - (i as isize * 4);
                    writeln!(output, "    lw {}, {}(sp)", reg, offset)?;
                }

                writeln!(output, "    lw ra, {}(sp)", ra_offset)?;
                writeln!(output, "    addi sp, sp, {}", frame_size)?;
                writeln!(output, "    ret\n")?;
            }

            Op::Label(label) => {
                writeln!(output, "{}:", label)?;
            }

            Op::Jump(target) => {
                writeln!(output, "    j {}", target)?;
            }

            Op::BranchIfFalse(target) => {
                let cond_reg = allocation.get(&instr.args[0]).unwrap();
                writeln!(output, "    beqz {}, {}", cond_reg, target)?;
            }

            Op::BranchCond(op, label) => {
                let lhs = allocation.get(&instr.args[0]).unwrap();
                let rhs = allocation.get(&instr.args[1]).unwrap();
                match op {
                    CmpOp::Eq => writeln!(output, "    bne {}, {}, {}", lhs, rhs, label)?,
                    CmpOp::Ne => writeln!(output, "    beq {}, {}, {}", lhs, rhs, label)?,
                    CmpOp::Lt => writeln!(output, "    bge {}, {}, {}", lhs, rhs, label)?,
                    CmpOp::Ge => writeln!(output, "    blt {}, {}, {}", lhs, rhs, label)?,
                    CmpOp::Le => writeln!(output, "    blt {}, {}, {}", rhs, lhs, label)?,
                    CmpOp::Gt => writeln!(output, "    bge {}, {}, {}", rhs, lhs, label)?,
                }
            }
            
            Op::Add => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs1 = allocation.get(&instr.args[0]).unwrap();
                let rs2 = allocation.get(&instr.args[1]).unwrap();
                writeln!(output, "    add {}, {}, {}", rd, rs1, rs2)?;
            }
            
            Op::Sub => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs1 = allocation.get(&instr.args[0]).unwrap();
                let rs2 = allocation.get(&instr.args[1]).unwrap();
                writeln!(output, "    sub {}, {}, {}", rd, rs1, rs2)?;
            }
            
            Op::Mul => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs1 = allocation.get(&instr.args[0]).unwrap();
                let rs2 = allocation.get(&instr.args[1]).unwrap();
                writeln!(output, "    mul {}, {}, {}", rd, rs1, rs2)?;
            }
            
            Op::Div => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs1 = allocation.get(&instr.args[0]).unwrap();
                let rs2 = allocation.get(&instr.args[1]).unwrap();
                writeln!(output, "    div {}, {}, {}", rd, rs1, rs2)?;
            }
            
            Op::Rem => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs1 = allocation.get(&instr.args[0]).unwrap();
                let rs2 = allocation.get(&instr.args[1]).unwrap();
                writeln!(output, "    rem {}, {}, {}", rd, rs1, rs2)?;
            }
            
            Op::And => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs1 = allocation.get(&instr.args[0]).unwrap();
                let rs2 = allocation.get(&instr.args[1]).unwrap();
                writeln!(output, "    and {}, {}, {}", rd, rs1, rs2)?;
            }
            
            Op::Or => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs1 = allocation.get(&instr.args[0]).unwrap();
                let rs2 = allocation.get(&instr.args[1]).unwrap();
                writeln!(output, "    or {}, {}, {}", rd, rs1, rs2)?;
            }
            
            Op::Xor => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs1 = allocation.get(&instr.args[0]).unwrap();
                let rs2 = allocation.get(&instr.args[1]).unwrap();
                writeln!(output, "    xor {}, {}, {}", rd, rs1, rs2)?;
            }
            
            Op::Sll => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs1 = allocation.get(&instr.args[0]).unwrap();
                let rs2 = allocation.get(&instr.args[1]).unwrap();
                writeln!(output, "    sll {}, {}, {}", rd, rs1, rs2)?;
            }
            
            Op::Srl => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs1 = allocation.get(&instr.args[0]).unwrap();
                let rs2 = allocation.get(&instr.args[1]).unwrap();
                writeln!(output, "    srl {}, {}, {}", rd, rs1, rs2)?;
            }
            
            Op::Neg => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs = allocation.get(&instr.args[0]).unwrap();
                writeln!(output, "    neg {}, {}", rd, rs)?;  
            }
            
            Op::Not => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs = allocation.get(&instr.args[0]).unwrap();
                writeln!(output, "    not {}, {}", rd, rs)?;
            }
        }
    }

    Ok(output)
}
