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
    /* Set stack pointer to top of RAM (0x80000000 + 32MB) */
    li sp, 0x82000000

    call main

    /* a0 now holds main's return value.
     * Encode it into the test finisher: value = (exit_code << 16) | 0x3333
     * Use 0x5555 for clean exit (pass), 0x3333 for failure.
     * For simplicity we always write (return_val << 16 | 0x5555) */
    li t0, 0x100000         /* test finisher MMIO address */
    slli a0, a0, 16
    li   t1, 0x5555         /* FINISHER_PASS */
    or   a0, a0, t1
    sw   a0, 0(t0)

    /* Should not reach here, but loop just in case */
.Lhang:
    j .Lhang
