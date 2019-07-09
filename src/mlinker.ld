OUTPUT_ARCH( "riscv" )
ENTRY( _start )

SECTIONS
{
  . = 0x80200000;
  .payload :
  {
    *(.payload)
  }

  /*
     Seek backwards to the start of memory. Any additional sections will be
     appended directly after this one, and an error will be generated if there
     are any overlaps with the payload.
  */
  . = 0x80000000;
  .text.entrypoint : AT(0x80000000)
  {
    *(.text.entrypoint)
  }
}
