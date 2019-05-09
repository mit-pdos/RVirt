OUTPUT_ARCH( "riscv" )
ENTRY( _start )

SECTIONS
{
  . = 0x80000000;
  .text.machine :
  {
    *(.text.machine)
  }
  . = ALIGN(0x1000);
  .rodata.machine :
  {
	*(.rodata.machine)
  }

  . = 0xffffffffc0100000;
  .text : AT(0x80100000)
  {
    *(.text.supervisor)
  }

  . = ALIGN(0x1000);
  .rodata :
  {
	*(.rodata.supervisor)
  }

  . = 0xffffffffc0200000;
  .shared.data : {
    *(.shared.data)
  }

  . = 0xffffffffc0400000;
  .data :
  {
    *(.data)
  }

  .bss :
  {
    *(.bss)
  }

  ASSERT(. < 0xffffffffc0600000, "")

  . = 0xffffffffe0000000;
  .initrd :
  {
	*(.initrd)
  }
}
