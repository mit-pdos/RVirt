# SVPN

SVPN is an S-mode trap-and-emulate hypervisor for RISC-V. It is currently targeted at QEMU's virt machine type and provides an M-mode stub so that it doesn't have to rely on BBL. It is designed to be powerful enough to run Linux as a guest operating system. 

## FAQ

### Why RISC-V?

RISC-V is [classically virtualizable](https://en.wikipedia.org/wiki/Popek_and_Goldberg_virtualization_requirements) which means that a hypervisor can rely on any privileged instruction triggering an illegal instruction fault when executed by the (unprivileged) guest OS. This is in constrast to other ISAs like x86 which have instructions that behave differently in user and kernel mode but never trap. Additionally, RISC-V has only 12 privileged control registers and only a handful of privileged instructions making the work to implement trap and emulate much more managable. 

### Why Rust?

Why not? Rust is a pleasant language to work with and can directly target bare metal systems. Although I had hoped otherwise, safety turned out not to be a big factor as nearly all the code turned out to directly or indirectly rely on unsafe.  

## Instructions

SVPN depends on a recent nightly version of rust installed via rustup, along with support for the `riscv64imac-unknown-none-elf` target. You will also need to install cross compilation tools for your system, which will include the `riscv64-unknown-elf-ld` linker. 

    $ sudo apt-get install binutils-riscv64-linux-gnu qemu-system-misc
    $ git clone https://github.com/fintelia/svpn && cd svpn
    $ rustup target add riscv64imac-unknown-none-elf
    $ make

To actually run SVPN, you'll need a guest binary. Grab the Fedora `vmlinux` kernel image from [here](https://fedorapeople.org/groups/risc-v/disk-images/) and place it in root of the repository. Now you can run with:

    $ make qemu

## Current Status

SVPN can currently boot a Linux guest right up until it switches away from the early boot console (at which point it stops producing output...) 

### Correctness

- [x] Trap and emulate of privileged instructions (CSR related and SFENCE.VMA)
- [x] Shadow page tables
- [ ] Update PTE accessed and dirty bits
- [ ] Timers
- [ ] Expose and/or emulate peripherals

### Functionality
Some features I'd like to have but not neccessary for correct virtualization of a single guest:

- [ ] multicore and inter-processor interrupts
- [ ] multiple guests

Other features not used by Linux are unlikely to be implemented:
- [ ] ASID support
- [ ] Sv48 or Sv57 page tables (only Sv39 currently allowed)
