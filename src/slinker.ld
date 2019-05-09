OUTPUT_ARCH( "riscv" )
ENTRY( sstart )

SECTIONS
{
  . = 0xffffffffc0100000;
  .text.supervisor : AT(0x80100000)
  {
    *(.text.entrypoint)
    *(.text) *(.text.*)
    *(.gnu.linkonce.t.*)
  }

  . = ALIGN(0x1000);
  .rodata.supervisor :
  {
    *(.rdata) *(.rodata) *(.rodata.*)
    *(.gnu.linkonce.r.*)
  }

  . = 0xffffffffc0200000;
  .shared.data : {
    *(.shared.data)
  }

  . = 0xffffffffc0400000;
  .data :
  {
    *(.data)
    *(.data.*)
    *(.srodata*)
    *(.gnu.linkonce.d.*)
    *(.comment)
  }

  .bss :
  {
    *(.bss)
    *(.bss.*)
    *(.sbss*)
    *(.gnu.linkonce.b.*)
    *(COMMON)
  }

  ASSERT(. < 0xffffffffc0600000, "")

  . = 0xffffffffe0000000;
  .initrd :
  {
  	*(.initrd)
  }
}
