# RVirt

RVirt is an S-mode trap-and-emulate hypervisor for RISC-V. It is currently targeted at QEMU's virt machine type and provides an M-mode stub so that it doesn't have to rely on BBL. It is designed to be powerful enough to run Linux as a guest operating system.

## FAQ

### Why RISC-V?

RISC-V is [classically virtualizable](https://en.wikipedia.org/wiki/Popek_and_Goldberg_virtualization_requirements) which means that a hypervisor can rely on any privileged instruction triggering an illegal instruction fault when executed by the (unprivileged) guest OS. This is in constrast to other ISAs like x86 which have instructions that behave differently in user and kernel mode but never trap. Additionally, RISC-V has only 12 privileged control registers and only a handful of privileged instructions making the work to implement trap and emulate much more managable.

### Why Rust?

Why not? Rust is a pleasant language to work with and can directly target bare metal systems. Although I had hoped otherwise, safety turned out not to be a big factor as nearly all the code turned out to directly or indirectly rely on unsafe.

## Instructions

RVirt depends on a recent nightly version of rust installed via rustup, along with support for the `riscv64imac-unknown-none-elf` target. You will also need to install cross compilation tools for your system, which will include the `riscv64-unknown-elf-ld` linker.

    $ sudo apt-get install binutils-riscv64-linux-gnu qemu-system-misc
    $ git clone https://github.com/fintelia/rvirt && cd rvirt
    $ rustup target add riscv64imac-unknown-none-elf
    $ make release

To actually run RVirt, you'll need a guest binary. Grab the Fedora `vmlinux` kernel binary and associated `stage4-disk.img` disk image from [here](https://fedorapeople.org/groups/risc-v/disk-images/) and place them in root of the repository. Now you can run with:

    $ make qemu-release

## Current Status

RVirt can currently boot a Linux guest until it starts systemd. Once started, systemd prints a small amount of output and then hangs.

### Correctness

- [x] Trap and emulate of privileged instructions (CSR related and SFENCE.VMA)
- [x] Shadow page tables
- [x] Update PTE accessed and dirty bits
- [x] Timers
- [x] Expose and/or emulate peripherals
- [ ] Address lingering bugs in boot process

### Functionality
Some features I'd like to have but not neccessary for correct virtualization of a single guest:

- [ ] multicore and inter-processor interrupts
- [ ] multiple guests
- [ ] PCIe devices

Other features not used by Linux are unlikely to be implemented:
- [ ] ASID support
- [ ] Sv48 or Sv57 guest page tables (only Sv39 currently allowed)
