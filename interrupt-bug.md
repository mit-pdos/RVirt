The bug manifested itself as a random guest crash due to a page fault, illegal
exception fault, or other strange error and the cause wasn't consistent between
runs. At first the only hint I had was that decreasing the timer clock frequency
(causing fewer timer interrupts to be delivered) caused the guest to get much
farther in its execution before crashing.

I suspected that this might because caused by a subtle bug in the way timer
interrupts were delivered to the Linux guest. If I was mistakenly leaving
interrupts enabled when the shouldn't have been I might deliver an interrupt
while Linux was inside a critical section. Vectoring to the wrong address,
incorrectly setting the privilege mode, or any of numerous other spec violations
could have caused Linux to behave erratically. Yet, going through my code line
by line exactly matched the RISC-V specification.

Next I tried running my code a bunch of times to see if I could find any
patterns in where the traps occurred. Printing out the instruction address where
each crash happened proved to be of little help. Nearly every time landed in a
different place, and often in entirely different functions.

By commenting out sections of hypervisor code, I was able to establish that
masking timer interrupts for Linux made the problem go away. Yet, this wasn't
conclusive: without timer interrupts Linux livelocked early in the boot process.

Eventually I noticed that if I delivered timer interrupts somewhere between
10-100x slower than real time then the crash would always happen in exactly the
same place and with the same error: "guest page table specified invalid physical
address". This consistency would prove invaluable in tracking down exactly what
was going on.

Examining disassebled output of the particular function, I learned that the
crash was happening in code generated from macros, meaning that no source was
available, and the error message itself proved to be a red herring: on RISC-V
Linux defaults to 128 GB of direct mapped physical addreses into the upper
portion of the page table, regardless of how much RAM is actually present. The
code had just happened to touch the first page past the end of RAM.

I tried stepping through the code with GDB and eventually started to understand
what was going on. The load instruction that triggered the fault was happening
inside a loop, and at a random iteration the loop bound (stored on the stack)
was being overwritten. Finally! This sort of memory corruption was consistent
with the crashes I was seeing.

It took me a while to notice that it wasn't actually the memory location that
was changing. Rather a single nibble in the frame pointer register was changing
so it went from 0xffffffe022057ce0 to 0xffffffe022057c70. Now it was simply a
matter of finding where it was being overwritten. I grepped through the
disassembly of my hypervisor and discovered that the only place were the frame
pointer was touched was to save it on trap entry and restore it on exit. The
compiler wasn't even emitting any code that used the frame pointer!

No matter, maybe somewhere I was overwritting the saved value on the stack with
another value? With some printf debugging I learned that this wasn't happening:
the saved value did not change at any point during my hypervisor's interrupt
handler. Then I noticed something even stranger, the frame pointer was already
corrupt when entering the hypervisor. I turned my attention the M-mode stub I
was using to forward interrupts to my hypervisor. Only 24 hand written assembly
instructions long, I couldn't see anything that might explain the bug. There was
no mention of the frame pointer register, and the few memory accesses were all
to safe addresses. I confirmed with objdump that the assembly produced matched
what I intended. GDB was initially uncooperative, but eventually I set a
breakpoint at the start of the M-mode stub and confirmed that the frame pointer
was already wrong even then!

At this point I was starting to get suspicious of QEMU. How could a register
change between two unrelated instructions? Running QEMU inside GDB I was able to
set a watchpoint on the location of the fp register, and saw that it changed
inside `cpu_tb_exec` while QEMU was running a translation block. The
modification was also happening *before* the breakpoint I'd set on mtrap_entry,
the start of my M-mode trap handler stub. This didn't make any sense, that
breakpoint should have fired before executing any code after the interrupt, yet
some code was being run that modified the register.

Once I noticed that the interrupt vector wasn't actually being set to
mtrap_entry I was able to start piecing together what was going on. I initialize
`mtvec` during boot with this bit of code:

```rust
    asm!("auipc t0, 0
          c.addi t0, 18
          csrw 0x305, t0 // mtvec
          c.j continue

.align 4
mtrap_entry:
          csrw 0x340, sp // mscratch

          ...

          mret
continue:" ::: "t0"  : "volatile");
...
```

Which the assembler expands to:

```
    8000006a:   00000297                auipc   x5,0x0
    8000006e:   02c1                    addi    x5,x5,18
    80000070:   30529073                csrw    mtvec,x5
    80000074:   a88d                    j       800000e6 <continue>
    80000076:   00000013                nop
    8000007a:   00000013                nop
    8000007e:   0001                    nop

0000000080000080 <mtrap_entry>:
    80000080:   34011073                csrw    mscratch,x2
    ...
00000000800000e6 <continue>:
```

This was intended to do was to set the machine mode trap vector, `mtvec`, to
0x80000080 (the address of mtrap_entry) and then jump over the code for the trap
handler to continue on with initialization. Unfortunately, the address
calculation was wrong and `mtvec` was actually initialized with the value
0x8000007c. At first glance this difference seems completely innocuous: that
other address land right in the middle of a range of NOPs and execution should
fall through to mtrap_entry. However, this is not what happens, because when the
processor jumps to that address it actually in the middle of a NOP and thus sees
a slightly different sequence of instructions:

```
    8000007c:   0000                    c.unimp
    8000007e:   0001                    nop
    80000080:   34011073                csrw    mscratch,x2
```

When execution reached the `c.unimp` instruction, the processor should have
triggered an illegal instruction exception and jumped back to the start of the
M-mode trap handler where it would have encountered the same illegal instruction
and looped forever. However, due to a bug in QEMU, the instruction sequence was
actually decoded as:

```
    8000007c:   0000                    c.addi4sp x8,0
    8000007e:   0001                    nop
    80000080:   34011073                csrw    mscratch,x2
```

Or, equivalently with aliases:

```
    8000007c:   0000                    mv      fp,sp
    8000007e:   0001                    nop
    80000080:   34011073                csrw    mscratch,sp
```

In other words, instead of trapping again, QEMU clobbered the frame pointer
(with the contents of the stack pointer) and then resumed execution. The
hypervisor trap handler then went on to save and restored this now invalid frame
pointer, and was thus unaffected by this corruption.

Linux was less lucky. Since the kernel was compiled with
`-fno-omit-frame-pointer`, it made heavy use of the register. However, RISC-V
has a large number of general purpose registers, so most of the time local
variables do not spill out onto the stack and thus most accesses to the frame
pointer only occur in function prologues/epilogues.

One notable exception is the raid6_int2_xor_syndrome function which benchmarks a
RAID algorithm that requires lots of registers. This function is also notable
because Linux runs it repeatedly until a specific number of timer interrupts
arrive (to measure how long it takes). This is also why I was able to
consistently get Linux to crash in the function. Here, the loop bound is stored
on the stack to free up a register:

```
ffffffe0002b8f70 <raid6_int2_xor_syndrome+0x4c>:
...
ffffffe0002b908a:   2ac1              addiw  s5,s5,16
...
ffffffe0002b90a6:   f9843783          ld     a5,-104(fp)
ffffffe0002b90aa:   ecfae3e3          bltu	 s5,a5,ffffffe0002b8f70 <raid6_int2_xor_syndrome+0x4c>
```

When a timer interrupt arrives while the program counter is in the body of the
loop, the frame pointer gets set to the stack pointer and so during the next
iteration instead of pulling off the correct loop bound it reads a garbage value
off the stack. In this particular case, that garbage value happened to be a
pointer to kernel memory (ie with MSB set) so instead of running `4096/16=256`
iterations of the loop it actually tried to run nearly `2^64/16=2^60` iterations
eventually running off the end of RAM.
