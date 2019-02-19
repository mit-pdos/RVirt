OUTPUT_ARCH( "riscv" )
ENTRY( _start )

SECTIONS
{
  . = 0x80000000;
  .text.init :
  {
    *(.text.init)
  }

  /* reserve space for boot page tables */
  ASSERT(. <= 0x80010000, "")

  . = 0x40200000;
  .text : AT(0x80200000)
  {
    *(.text.strap_entry)
    *(.text) *(.text.*)
    *(.gnu.linkonce.t.*)
  }

  .rodata :
  {
    *(.rdata) *(.rodata) *(.rodata.*)
    *(.gnu.linkonce.r.*)
  }

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

  ASSERT(. < 0x80400000, "")

  mtrap_entry_offset = mtrap_entry - 0x80000000;
}
