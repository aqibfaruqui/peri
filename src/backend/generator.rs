use crate::ir::{Instruction, Op};
use crate::backend::regalloc::Allocation;
use std::fmt::Write;

pub fn generate(
    func_name: &str, 
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
    writeln!(output, ".global {}", func_name)?;
    writeln!(output, "{}:", func_name)?;

    // TODO: Calculate necessary stack offset from function arguments
    writeln!(output, "    addi sp, sp, -16")?;
    writeln!(output, "    sw ra, 12(sp)")?;

    for instr in instructions {
        match &instr.operation {
            Op::LoadImm(val) => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                // TODO: Check portability of 'li' pseudoinstruction
                writeln!(output, "    li {}, {}", rd, val)?;
            }

            Op::Mov => {
                let rd = allocation.get(&instr.destination.unwrap()).unwrap();
                let rs = allocation.get(&instr.args[0]).unwrap();
                // TODO: Check portability of 'mv' pseudoinstruction
                writeln!(output, "    mv {}, {}", rd, rs)?;
            }

            Op::Call(target) => {
                // TODO: Handle args (move to a0...a7)
                // TODO: Check portability of 'call' pseudoinstruction
                writeln!(output, "    call {}", target)?;
                if let Some(dest) = instr.destination {
                    let rd = allocation.get(&dest).unwrap();
                    writeln!(output, "    mv {}, a0", rd)?;
                }
            }

            Op::Ret => {
                // TODO: Move a return value to a0 
                // TODO: Check portability of 'ret' pseudoinstruction
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
        }
    }

    Ok(output)
}
