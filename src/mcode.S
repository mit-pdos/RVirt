
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
.align 4
mtrap_entry:
	csrrw sp, mscratch, sp
	addi sp, sp, -128
	sd ra, 0(sp)
	sd t0, 8(sp)
	sd t1, 16(sp)
	sd t2, 24(sp)
	sd t3, 32(sp)
	sd t4, 40(sp)
	sd t5, 48(sp)
	sd t6, 56(sp)
	sd a0, 64(sp)
	sd a1, 72(sp)
	sd a2, 80(sp)
	sd a3, 88(sp)
	sd a4, 96(sp)
	sd a5, 104(sp)
	sd a6, 112(sp)
	sd a7, 120(sp)

	csrr t0, mcause
	bgez t0, exception

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
	csrc mip, t0 // mip.mtip = 0

	li t0, 0x02
	csrs mip, t0 // mip.stip = 1

	csrr t0, mhartid
	slli t0, t0, 3
	li t1, 0x2004000
	add t1, t0, t1
	li t0, 0xffffffffffff
	sd t0, 0(t1)  // mtimecmp[hartid] = 2^48 - 1

	j return

mexternal_interrupt:
	j mexternal_interrupt

exception:
	call forward_exception
	j return

return:
	ld ra, 0(sp)
	ld t0, 8(sp)
	ld t1, 16(sp)
	ld t2, 24(sp)
	ld t3, 32(sp)
	ld t4, 40(sp)
	ld t5, 48(sp)
	ld t6, 56(sp)
	ld a0, 64(sp)
	ld a1, 72(sp)
	ld a2, 80(sp)
	ld a3, 88(sp)
	ld a4, 96(sp)
	ld a5, 104(sp)
	ld a6, 112(sp)
	ld a7, 120(sp)
	addi sp, sp, 128
	csrrw sp, 0x340, sp // mscratch
	mret

// This trap handler is set for harts that do not yet have guest VM running on
// them. Once hart 0 has initialized all necessary state for a VM, it sends a
// software interrupt which will cause this trap handler to run. This handler
// installs a new trap handler and then calls into the S-mode hypervisor code.
.align 4
start_hart:
	csrr t0, mcause
	li t1, 0x8000000000000003
	bne t0, t1, unknown_cause

	// Install new mtvec
	LOAD_ADDRESS t0, mtrap_entry
	csrw mtvec, t0

	// Set msip = 0
	csrr a0, mhartid
	li t0, 0x02000000
	slli t1, a0, 2
	add t0, t0, t1
	sw zero, 0(t0)

	// Set mstatus.mpp[1] = 0
	li t0, 0x1000
	csrc mstatus, t0

	call handle_ipi
