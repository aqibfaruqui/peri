/*
 * Linker script for QEMU virt machine (RISC-V 32-bit)
 *
 * QEMU virt memory map:
 *   0x00000000 - QEMU ROM / boot ROM
 *   0x00100000 - Test finisher device (for QEMU exit)
 *   0x80000000 - Main RAM (starts here, we load kernel here)
 *
 * We place _start (via .text.start) first so QEMU begins execution there.
 */

ENTRY(_start)

MEMORY {
    RAM (rwx) : ORIGIN = 0x80000000, LENGTH = 32M
}

SECTIONS {
    . = 0x80000000;

    /* Startup code must come first */
    .text.start : {
        *(.text.start)
    } > RAM

    /* Rest of code */
    .text : {
        *(.text .text.*)
    } > RAM

    /* Read-only data */
    .rodata : {
        *(.rodata .rodata.*)
    } > RAM

    /* Initialised data */
    .data : {
        *(.data .data.*)
    } > RAM

    /* Zero-initialised data */
    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    } > RAM

    /* Stack grows down from end of RAM — start.S sets sp explicitly */
}
