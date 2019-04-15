
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

// This is the default M-mode trap handler. It forwards timer interrupts to
// S-mode and loops for all other interrupt and exception causes.
.section .text.mcode
.align 4
mtrap_entry:
	csrrw sp, 0x340, sp // mscratch
	sd t0, 0(sp)
	sd t1, 8(sp)

	csrr t0, 0x342 // mcause
	li t1, 0x8000000000000003
	beq t0, t1, msoftware_interrupt

	li t1, 0x8000000000000007
	beq t0, t1, mtimer_interrupt

	li t1, 0x800000000000000b
	beq t0, t1, mexternal_interrupt

unknown_cause:
	j unknown_cause

msoftware_interrupt:
	j msoftware_interrupt

mtimer_interrupt:
	li t0, 0x80
	csrc 0x344, t0 // mip.mtip = 0

	li t0, 0x20
	csrs 0x144, t0 // sip.stip = 1

	csrr t0, 0xf14 // mhartid
	slli t0, t0, 3
	li t1, 0x2004000
	add t1, t0, t1
	li t0, 0xffffffffffff
	sd t0, 0(t1)  // mtimecmp[hartid] = 2^48 - 1

	j return

mexternal_interrupt:
	j mexternal_interrupt

return:
	ld t0, 0(sp)
	ld t1, 8(sp)
	csrrw sp, 0x340, sp // mscratch
	mret

// This trap handler is set for harts that do not yet have guest VM running on
// them. Once hart 0 has initialized all necessary state for a VM, it sends a
// software interrupt which will cause this trap handler to run. This handler
// installs a new trap handler and then calls into the S-mode hypervisor code.
.align 4
start_hart:
	csrr t0, 0x342 // mcause
	li t1, 0x8000000000000003
	bne t0, t1, unknown_cause

	// Install new mtvec
	LOAD_ADDRESS t0, mtrap_entry
    csrw 0x305, t0

	// Load mhartid into a0
	csrr a0, 0xf14

	// Set msip = 0
	li t0, 0x02000000
	slli t1, a0, 2
	add t0, t0, t1
	sw zero, 0(t0)

	// Set mepc to hart_entry
	LOAD_ADDRESS ra, hart_entry
	csrw 0x341, ra

	// Set mstatus.mpp[1] = 0
	li t0, 0x1000
	csrc 0x300, t0

	mret
