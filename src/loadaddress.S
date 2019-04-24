
// LLVM's assembler is currently buggy and cannot produce position independent
// code or static code that references addresses which aren't sign extended
// 32-bit ints. Unfortunately none of physical memory falls into that
// range. This macro helps work around the bug by using a hacky trick to support
// addresses in the first couple MBs above 0x800000000.
//
// Uses of this macro can be replaced by the `la rd, symbol` psuedo-instruction
// where supported.
.macro LOAD_ADDRESS rd, symbol
	lui \rd, %hi(\symbol - (2047<<12))
	srli \rd, \rd, 12
	addi \rd, \rd, 2047
	slli \rd, \rd, 12
	addi \rd, \rd, %lo(\symbol - (2047<<12))
.endm