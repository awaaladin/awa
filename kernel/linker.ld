/* AWA kernel linker script.
 * Each segment has exactly the permissions it needs (W^X): code is R+X,
 * read-only data is R, data and Limine requests are R+W. */
OUTPUT_FORMAT(elf64-x86-64)
ENTRY(kmain)

PHDRS
{
    requests PT_LOAD FLAGS((1 << 1) | (1 << 2)); /* R W */
    text     PT_LOAD FLAGS((1 << 0) | (1 << 2)); /* R X */
    rodata   PT_LOAD FLAGS((1 << 2));            /* R   */
    data     PT_LOAD FLAGS((1 << 1) | (1 << 2)); /* R W */
}

SECTIONS
{
    /* Top 2 GiB of the address space, as required by the kernel code model. */
    . = 0xffffffff80000000;

    .requests : {
        KEEP(*(.requests_start_marker))
        KEEP(*(.requests))
        KEEP(*(.requests_end_marker))
    } :requests

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    .text : {
        *(.text .text.*)
    } :text

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    .rodata : {
        *(.rodata .rodata.*)
    } :rodata

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    .data : {
        *(.data .data.*)
    } :data

    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    } :data

    /DISCARD/ : {
        *(.eh_frame*)
        *(.note .note.*)
    }
}
