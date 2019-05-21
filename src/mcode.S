
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
	csrsi mip, 0x2 // mip.ssip = 1

	csrr t0, mhartid
	slli t0, t0, 2
	li t1, 0x2000000
	add t1, t0, t1
	sw zero, 0(t1)  // mtimecmp[hartid] = zero

	j return

mtimer_interrupt:
	li t0, 0x20
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
	li t1, 9
	beq t0, t1, ecall_exception
	call forward_exception
	j return

ecall_exception:
	csrr t0, mepc
	addi t0, t0, 4
	csrw mepc, t0

	beqz a7, sbi_set_timer

	li t1, 3
	beq a7, t1, sbi_clear_ipi

	li t1, 4
	beq a7, t1, sbi_send_ipi

	li t1, 8
	beq a7, t1, sbi_shutdown

	j unknown_cause

sbi_set_timer:
	csrr t0, mhartid
	slli t0, t0, 3
	li t1, 0x2004000
	add t1, t0, t1
	sd a0, 0(t1)  // mtimecmp[hartid] = a0

	li t0, 0x20
	csrc mip, t0 // mip.stip = 0

	li a0, 0
	j return_with_value

sbi_clear_ipi:
	csrci mip, 0x2
	li a0, 0
	j return_with_value

sbi_send_ipi:
	ld t0, 0(a0)
	li t2, (0x02000000 - 4)
1:	addi t2, t2, 4
	andi t1, t0, 0x1
	srli t0, t0, 1
	beqz t1, 1b
	sw t1, 0(t2)
	bnez t0, 1b

	li a0, 0
	j return_with_value

sbi_shutdown:
	j sbi_shutdown

return:
	ld a0, 64(sp)
return_with_value:
	ld ra, 0(sp)
	ld t0, 8(sp)
	ld t1, 16(sp)
	ld t2, 24(sp)
	ld t3, 32(sp)
	ld t4, 40(sp)
	ld t5, 48(sp)
	ld t6, 56(sp)
	ld a1, 72(sp)
	ld a2, 80(sp)
	ld a3, 88(sp)
	ld a4, 96(sp)
	ld a5, 104(sp)
	ld a6, 112(sp)
	ld a7, 120(sp)
	addi sp, sp, 128
	csrrw sp, mscratch, sp
	mret
