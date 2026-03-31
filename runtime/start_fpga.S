/* RISC-V Minimal Startup for QEMU virt machine
 *
 * QEMU virt starts execution at 0x80000000. This file:
 *   1. Sets up the stack pointer
 *   2. Calls main()
 *   3. Writes the return value to the QEMU test finisher device to exit cleanly
 *
 * QEMU virt test finisher:
 *   Base address: 0x100000
 *   Write 0x5555 to pass (exit 0), 0x3333 to fail
 *   Upper 16 bits encode the exit code
 */

.section .text.start
.global _start

_start:
    /* Set stack pointer to top of System RAM */
    la sp, __machine_stack_top
    li a0, 0    // TODO Finish

    call main

.Lhang:
    j .Lhang
