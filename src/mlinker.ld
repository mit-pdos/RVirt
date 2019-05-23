OUTPUT_ARCH( "riscv" )
ENTRY( _start )

SECTIONS
{
  . = 0x80000000;
  .text.machine :
  {
    *(.text.entrypoint)
    *(.text) *(.text.*)
    *(.gnu.linkonce.t.*)
  }

  . = ALIGN(0x1000);
  _rodata_pa = .;
  . += 0xffffffff40000000;
  .rodata.machine : AT(_rodata_pa)
  {
    *(.rdata) *(.rodata) *(.rodata.*)
    *(.gnu.linkonce.r.*)
    *(.srodata*)
    *(.comment)
  }

  . = ALIGN(0x200000);
  .payload : {
    *(.payload)
  }

  /DISCARD/ :
  {
    *(.shared.data)

    *(.data)
    *(.data.*)
    *(.gnu.linkonce.d.*)

	*(.bss)
    *(.bss.*)
    *(.sbss*)
    *(.gnu.linkonce.b.*)
    *(COMMON)
  }
}
