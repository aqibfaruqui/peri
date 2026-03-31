/*
 * Linker script for Lab FPGA Board (RISC-V 32-bit)
 *
 * Physical memory map:
 *   0x0000_0000 - System RAM
 *   0x0001_0000 - I/O Devices
 *   0x0004_0000 - User RAM
 *
 * We place _start (via .text.start) first so QEMU begins execution there.
 */

ENTRY(_start)

MEMORY {
    RAM (rwx) : ORIGIN = 0x00000000, LENGTH = 16K
}

SECTIONS {
    . = 0x00000000;

    /* Startup code must come first */
    .text.start : {
        *(.text.start)
    } > RAM

    /* Rest of code */
    .text : {
        *(.text .text.*)
    } > RAM

    /* Top of OS-Space Stack */
    . = 0x00010000;
    __machine_stack_top = .;

    /* Stack grows down from end of RAM — start.S sets sp explicitly */
}
