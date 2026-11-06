/* AWA kernel linker script.
 * Each segment has exactly the permissions it needs (W^X): code is R+X,
 * read-only data is R, data and Limine requests are R+W. The section
 * boundary symbols let the kernel map each part with the same rules in its
 * own page tables. */
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
    __kernel_start = .;

    .requests : {
        KEEP(*(.requests_start_marker))
        KEEP(*(.requests))
        KEEP(*(.requests_end_marker))
    } :requests

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __text_start = .;
    .text : {
        /* Entry code first, on its own pages: these pages stay mapped while
           user programs run (kernel page-table isolation). */
        __entry_start = .;
        KEEP(*(.entry_text))
        . = ALIGN(4096);
        __entry_end = .;
        *(.text .text.*)
    } :text
    __text_end = .;

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __rodata_start = .;
    .rodata : {
        *(.rodata .rodata.*)
    } :rodata
    __rodata_end = .;

    . = ALIGN(CONSTANT(MAXPAGESIZE));
    __data_start = .;
    .data : {
        *(.data .data.*)
    } :data

    .bss : {
        *(.bss .bss.*)
        *(COMMON)
    } :data
    . = ALIGN(4096);
    __kernel_end = .;

    /DISCARD/ : {
        *(.eh_frame*)
        *(.note .note.*)
    }
}
