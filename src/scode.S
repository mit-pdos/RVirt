.globl sstart
.section .text.entrypoint
sstart:
	// a2 = offset all code/data is shifted by ("shared_segment_shift")
	auipc a2, 0
	li t0, 0x80000000 // = SUPERVISOR_START_ADDRESS - SYMBOL_PA2VA_OFFSET
	sub a2, a2, t0

	// sp = M_MODE_STACK_BASE + hartid * M_MODE_STACK_STRIDE
	li sp, 0x80810000 // = M_MODE_STACK_BASE
	slli t0, a0, 16   // = a0 * M_MODE_STACK_STRIDE
    add sp, sp, t0

	// sp = pa2va(sp)
	li t0, 0xffffffff40000000 // = SYMBOL_PA2VA_OFFSET
	add sp, sp, t0

	// s4 = &boot_page_tables[hartid][0]
	li s4, 0x80200000 // s4 = 0x80200000
	add s4, s4, a2    //      + shared_segment_shift
	slli t0, a0, 13
	add s4, s4, t0    //      + 1024 * hartid

	// s5 = &boot_page_tables[hartid][511]
	li s5, 511 * 8
	add s5, s5, s4

	// s6 = &boot_page_tables[hartid][1024]
	li s6, 1024 * 8
	add s6, s6, s4

	// boot_page_tables[hartid][511..1024] += shared_segment_shift >> 2
	srli t0, a2, 21
	slli t0, t0, 19
1:  ld t1, 0(s5)
	add t1, t1, t0
	sd t1, 0(s5)
	addi s5, s5, 8
	blt s5, s6, 1b

	// stvec = trampoline
	lui t0, %hi(trampoline)
	addi t0, t0, %lo(trampoline)
	csrw stvec, t0

	// satp = &boot_page_tables[hartid][0] | 8 << 60
	srli t0, s4, 12
	li t1, 8 << 60
	or t0, t0, t1
	csrw satp, t0
	sfence.vma

.align 3
trampoline:
	lui t0, %hi(sstart2)
	addi t0, t0, %lo(sstart2)
	jr t0

.align 3
.globl hart_entry
hart_entry:
	csrr a0, sscratch
	j hart_entry2

.align 3
.globl panic_trap_handler
panic_trap_handler:
	call panic_trap_handler2
