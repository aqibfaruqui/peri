use crate::ir::{Instruction, Op};
use crate::ir::cfg::CmpOp;
use crate::backend::regalloc::Allocation;
use std::fmt::Write;

pub fn generate(
    function: &str, 
    instructions: &Vec<Instruction>, 
    allocation: &Allocation
) -> Result<String, std::fmt::Error> {
    let mut output = String::new();

    /*
     * .section .text
     * .global example_func
     * example_func:
     */
    writeln!(output, ".section .text")?;
    writeln!(output, ".global {}", function)?;
    writeln!(output, "{}:", function)?;

    // TODO: Calculate necessary stack offset from function arguments
    writeln!(output, "    addi sp, sp, -16")?;
    writeln!(output, "    sw ra, 12(sp)")?;

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

                // TODO: Update stack offsets with calculation of function arguments
                writeln!(output, "    lw ra, 12(sp)")?;
                writeln!(output, "    addi sp, sp, 16")?;
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
