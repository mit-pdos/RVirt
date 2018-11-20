OUTPUT_FORMAT("elf64-x86-64", "elf64-x86-64", "elf64-x86-64")
OUTPUT_ARCH(i386:x86-64)

ENTRY(start)

SECTIONS
{
	. = 0xFFFFFF0000001000;
	.multiboot : AT (0x100000) {
	        KEEP( *(.multiboot) )
	        *(.multiboot)
	}
	.text : {
		*(.text .stub .text.* .gnu.linkonce.t.*)
	}
	.rodata : {
		*(.rodata .rodata.* .gnu.linkonce.r.*)
	}

	. = ALIGN(0x1000);

        /* Conventionally, Unix linkers provide pseudo-symbols
         * etext, edata, and end, at the end of the text, data, and bss.
         * For the kernel mapping, we need the address at the beginning
         * of the data section, but that's not one of the conventional
         * symbols, because the convention started before there was a
         * read-only rodata section between text and data. */
        PROVIDE(data = .);
	.data : {
		*(.data .data.*)
	}
	bss : {
		PROVIDE(edata = .);
		*(.bss .bss.*)
		*(COMMON)
		PROVIDE(ebss = .);
	}
}
