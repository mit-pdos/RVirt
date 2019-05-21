.globl jump_to_sstart
.section .text.jump
jump_to_sstart:
	auipc t0, (0x100000 >> 12)
	jr t0

.globl sstart
.section .text.entrypoint
sstart:
	// a2 = real physical address of sstart
	auipc a2, 0

	// t1 = offset all code/data is shifted by
	li t1, 0x80100000 // = SUPERVISOR_START_ADDRESS - SYMBOL_PA2VA_OFFSET
	sub a2, a2, t1

	// initialize stack pointer
	li sp, 0x80810000 // = M_MODE_STACK_BASE
	li t2, 0x10000    // = M_MODE_STACK_STRIDE
	mul t0, a0, t2
    add sp, sp, t0

	li t2, 0xffffffff40000000 // = SYMBOL_PA2VA_OFFSET
	add sp, sp, t2

	// initialize and install page table
	srli t2, a2, 21
	slli t2, t2, 19
	li s5, 0x80200000 + 511 * 8
	li s6, 0x80200000 + 1024 * 8
	add s5, s5, a2
	add s6, s6, a2
1:  ld s0, 0(s5)
	add s0, s0, t2
	sd s0, 0(s5)
	addi s5, s5, 8
	blt s5, s6, 1b

	// Set trap vector to trampoline
	lui t0, %hi(trampoline)
	addi t0, t0, %lo(trampoline)
	csrw stvec, t0

	// set satp
	li s4, (8 << 60 | 0x80200000 >> 12)
	srli t1, a2, 12
	add s4, s4, t1
	csrw satp, s4
	sfence.vma

.align 3
trampoline:
	lui t0, %hi(sstart2)
	addi t0, t0, %lo(sstart2)
	jr t0

	call jump_to_sstart

.align 3
.globl hart_entry
hart_entry:
	csrr a0, sscratch
	j hart_entry2
