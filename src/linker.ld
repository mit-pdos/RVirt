OUTPUT_ARCH( "riscv" )
ENTRY( _start )

SECTIONS
{
  . = 0x80000000;
  .text.init :
  {
    *(.text.entrypoint)
    *(.text.init)
  }

  . = ALIGN(0x1000);
  _text_pa = .;
  . += 0xffffffff40000000;
  .text : AT(_text_pa)
  {
    *(.text) *(.text.*)
    *(.gnu.linkonce.t.*)
  }

  .rodata :
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
}
