
.macro LOAD_ADDRESS rd, symbol
          lui \rd, %hi(\symbol - (2047<<12))
          srli \rd, \rd, 12
          addi \rd, \rd, 2047
          slli \rd, \rd, 12
          addi \rd, \rd, %lo(\symbol - (2047<<12))
.endm

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
          li t0, 0x02000000
          csrr t1, 0xf14 // mhartid
          slli t1, t1, 2
          add t0, t0, t1
          sw zero, 0(t0)

          csrw 0x341, ra // mepc

          li t0, 0x1000
          csrc 0x300, t0 // mstatus.mpp[1] = 0

          csrr a0, 0xf14 // mhartid

          j return

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
