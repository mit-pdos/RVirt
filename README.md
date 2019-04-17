# RVirt

RVirt is an S-mode trap-and-emulate hypervisor for RISC-V. It is currently targeted at QEMU's virt machine type and provides an M-mode stub so that it doesn't have to rely on BBL. It is designed to be powerful enough to run Linux as a guest operating system.

## FAQ

### Why RISC-V?

RISC-V is [classically virtualizable](https://en.wikipedia.org/wiki/Popek_and_Goldberg_virtualization_requirements) which means that a hypervisor can rely on any privileged instruction triggering an illegal instruction fault when executed by the (unprivileged) guest OS. This is in constrast to other ISAs like x86 which have instructions that behave differently in user and kernel mode but never trap. Additionally, RISC-V has only 12 privileged control registers and only a handful of privileged instructions making the work to implement trap and emulate much more managable.

### Why Rust?

Why not? Rust is a pleasant language to work with and can directly target bare metal systems. Although I had hoped otherwise, safety turned out not to be a big factor as nearly all the code turned out to directly or indirectly rely on unsafe.

## Installing dependencies

 - rustup: https://rustup.rs/
   - customize configuration to select the "nightly" build during setup.
   - add the RISC-V target:

         $ rustup target add riscv64imac-unknown-none-elf

 - binutils (for cross-compilation):
   - if it's available on your distro:

         $ sudo apt-get install binutils-riscv64-linux-gnu

      You may have to change the makefile to use riscv64-linux-gnu-ld as the linker
      
   - if not:

         $ wget https://ftp.gnu.org/gnu/binutils/binutils-2.32.tar.xz
         $ tar -xf binutils-2.32.tar.xz binutils-2.32/
         $ cd binutils-2.32/
         $ ./configure --target=riscv64-unknown-elf --disable-nls
         $ make
         $ sudo make install

     Rust looks for "riscv64-unknown-elf", so don't use any of the other variants.
     I included --disable-nls for compilation speed; it's probably unnecessary.

 - qemu:
   - if the right version is available for your system (4.0.0-rc0 or greater):

         $ sudo apt-get install qemu-system-misc

   - if not:

         $ wget https://download.qemu.org/qemu-4.0.0-rc0.tar.xz
         $ tar -xf qemu-4.0.0-rc0.tar.xz qemu-4.0.0-rc0/
         $ cd qemu-4.0.0-rc0
         $ ./configure --target-list=riscv64-softmmu
         $ make
         $ sudo make install

 - gdb (optional):

   to download and build:

       $ wget https://ftp.gnu.org/gnu/gdb/gdb-8.2.1.tar.xz
       $ tar -xf gdb-8.2.1.tar.xz gdb-8.2.1/
       $ cd gdb-8.2.1
       $ ./configure --target=riscv64-unknown-elf --disable-nls
       $ make
       $ sudo make install

## Instructions

Download RVirt's source code:

    $ git clone https://github.com/fintelia/rvirt
    $ cd rvirt

Build RVirt:

    $ make release

You'll need guest binaries to run RVirt: a kernel binary (vmlinux) and a disk image (stage4-disk.img) from [here](https://fedorapeople.org/groups/risc-v/disk-images/).

      # make sure to be in the root of the repository
    $ wget https://fedorapeople.org/groups/risc-v/disk-images/vmlinux
    $ mv vmlinux fedora-vmlinux
    $ wget https://fedorapeople.org/groups/risc-v/disk-images/stage4-disk.img.xz
    $ unxz stage4-disk.img.xz

Now you can run with:

    $ make qemu

If you want to debug using gdb, run these commands in the project directory in separate shells:

    $ make qemu-gdb
    $ riscv64-unknown-elf-gdb

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
