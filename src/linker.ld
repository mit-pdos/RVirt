OUTPUT_ARCH( "riscv" )
ENTRY( _start )

SECTIONS
{
  . = 0x80000000;

  _init_start = .;
  .text.init : {
    *(.text.init)
  }
  . = ALIGN(0x1000);
  .data.statics : {
     *(.data.statics)
  }

  /* reserve space for boot page tables */
  . = 0x80010000;

  . = 0x80020000;
  .text.init2 : {
    *(.text.init2)
  }

  . = 0x80030000;
  .text.mtrap : {
     *(.text.mtrap)
  }

  . = 0x80200000;
  _init_end = .;

  . = 0x40200000;

  .text : AT(_init_end)
  {
    *(.text) *(.text.*)
    *(.gnu.linkonce.t.*)
  }

  .rodata :
  {
    *(.rdata) *(.rodata) *(.rodata.*)
    *(.gnu.linkonce.r.*)
  }

  /* End of code and read-only segment */
  PROVIDE( etext = . );
  _etext = .;

  /*--------------------------------------------------------------------*/
  /* HTIF, isolated onto separate page                                  */
  /*--------------------------------------------------------------------*/
  . = ALIGN(0x1000);
  .htif :
  {
    PROVIDE( __htif_base = . );
    *(.htif)
  }
  . = ALIGN(0x1000);

  /*--------------------------------------------------------------------*/
  /* Initialized data segment                                           */
  /*--------------------------------------------------------------------*/

  /* Start of initialized data segment */
  . = ALIGN(16);
   _fdata = .;

  /* data: Writable data */
  .data :
  {
    *(.data)
    *(.data.*)
    *(.srodata*)
    *(.gnu.linkonce.d.*)
    *(.comment)
  }

  /* End of initialized data segment */
  . = ALIGN(4);
  PROVIDE( edata = . );
  _edata = .;

  /*--------------------------------------------------------------------*/
  /* Uninitialized data segment                                         */
  /*--------------------------------------------------------------------*/

  /* Start of uninitialized data segment */
  . = .;
  _fbss = .;

  /* sbss: Uninitialized writeable small data section */
  . = .;

  /* bss: Uninitialized writeable data section */
  . = .;
  _bss_start = .;
  .bss :
  {
    *(.bss)
    *(.bss.*)
    *(.sbss*)
    *(.gnu.linkonce.b.*)
    *(COMMON)
  }

  . = ALIGN(0x1000);
  _end = .;

  mtrap_entry_offset = mtrap_entry - 0x80000000;
  strap_entry_offset = strap_entry - 0x80000000;
}
