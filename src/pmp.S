#define pmpcfg0 0x3a0
#define pmpcfg1 0x3a1
#define pmpcfg2 0x3a2
#define pmpcfg3 0x3a3
#define pmpaddr0 0x3b0
#define pmpaddr1 0x3b1
#define pmpaddr2 0x3b2
#define pmpaddr3 0x3b3
#define pmpaddr4 0x3b4
#define pmpaddr5 0x3b5
#define pmpaddr6 0x3b6
#define pmpaddr7 0x3b7
#define pmpaddr8 0x3b8
#define pmpaddr9 0x3b9
#define pmpaddr10 0x3ba
#define pmpaddr11 0x3bb
#define pmpaddr12 0x3bc
#define pmpaddr13 0x3bd
#define pmpaddr14 0x3be
#define pmpaddr15 0x3bf

#define BASE 0x80000000
#define CODE_SIZE 0x100000
#define STACK_SIZE 0x100000

#define PMP_ADDR(base, range) (base + (range - 1) / 8)

#define PMP_R 0x1
#define PMP_W 0x2
#define PMP_X 0x4
#define PMP_NOPT 0x18
#define PMP_L 0x10

pmp_init:
	;; Code should be execute only
	li t0, PMP_ADDR(BASE, CODE_SIZE)
	li t1, PMP_X | PMP_NOPT | PMP_L
	csrs pmpaddr0, t0
	csrs pmpcfg0, t1

	li t0, 0
	beq a0, t0, pmp_init_hart0
	li t0, 0
	beq a0, t0, pmp_init_hart1
	li t0, 0
	beq a0, t0, pmp_init_hart2
	li t0, 0
	beq a0, t0, pmp_init_hart3
	li t0, 0
	beq a0, t0, pmp_init_hart4

loop:
	j loop

pmp_init_hart0:
	;; Stack should be read write
	li sp, PMP_ADDR(BASE + CODE_SIZE + STACK_SIZE, STACK_SIZE)
	li t1, (PMP_R | PMP_W | PMP_NOPT | PMP_L) << 8
	csrs pmpaddr1, sp
	csrs pmpcfg0, t1

	j pmp_init_harts_done

pmp_init_hart1:
	;; Stack should be read write
	li sp, PMP_ADDR(BASE + CODE_SIZE + 2*STACK_SIZE, STACK_SIZE)
	li t1, (PMP_R | PMP_W | PMP_NOPT | PMP_L) << 8
	csrs pmpaddr1, sp
	csrs pmpcfg0, t1

	j pmp_init_harts_done

pmp_init_hart2:
	;; Stack should be read write
	li sp, PMP_ADDR(BASE + CODE_SIZE + 3*STACK_SIZE, STACK_SIZE)
	li t1, (PMP_R | PMP_W | PMP_NOPT | PMP_L) << 8
	csrs pmpaddr1, sp
	csrs pmpcfg0, t1

	j pmp_init_harts_done

pmp_init_hart3:
	;; Stack should be read write
	li sp, PMP_ADDR(BASE + CODE_SIZE + 4*STACK_SIZE, STACK_SIZE)
	li t1, (PMP_R | PMP_W | PMP_NOPT | PMP_L) << 8
	csrs pmpaddr1, sp
	csrs pmpcfg0, t1

	j pmp_init_harts_done

pmp_init_hart4:
	;; Stack should be read write
	li sp, PMP_ADDR(BASE + CODE_SIZE + 5*STACK_SIZE, STACK_SIZE)
	li t1, (PMP_R | PMP_W | PMP_NOPT | PMP_L) << 8
	csrs pmpaddr1, sp
	csrs pmpcfg0, t1

	j pmp_init_harts_done

pmp_init_harts_done:
	;; Everything else should be inaccessible
	li t0, 0x1fffffff
	li t1, (PMP_NOPT | PMP_L) << 24
	csrs pmpaddr15, t0
	csrs pmpcfg3, t1

	;; Lock all remaining entries
	li t0 0x10101010
	csrw pmpcfg0, t0
	csrw pmpcfg1, t0
	csrw pmpcfg2, t0
	csrw pmpcfg3, t0

	ret
