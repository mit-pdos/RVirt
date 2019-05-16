# RVirt

RVirt is an S-mode trap-and-emulate hypervisor for RISC-V. It is currently targeted at QEMU's virt machine type and provides an M-mode stub so that it doesn't have to rely on BBL. It is powerful enough to run Linux as a guest operating system.

## FAQ

### Why RISC-V?

RISC-V is [classically virtualizable](https://en.wikipedia.org/wiki/Popek_and_Goldberg_virtualization_requirements) which means that a hypervisor can rely on any privileged instruction triggering an illegal instruction fault when executed by the (unprivileged) guest OS. This is in constrast to other ISAs like x86 which have instructions that behave differently in user and kernel mode but never trap. Additionally, RISC-V has only 12 privileged control registers and only a handful of privileged instructions making the work to implement trap and emulate much more managable.

### Why Rust?

Why not? Rust is a pleasant language to work with and can directly target bare metal systems. I was also exited by Rust's ability to guarantee memory safety for safe code, but I found the amount of unsafe code required for initialization and vm entry/exit partially negated this benefit.

## Installing dependencies

 - rustup: https://rustup.rs/
   - customize configuration to select the "nightly" build during setup.

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

## Prepare guest operating system

You'll need guest binaries to run RVirt. The easiest option are to get a kernel binary (vmlinux) and a disk image (stage4-disk.img) from [here](https://fedorapeople.org/groups/risc-v/disk-images/):

      # make sure to be in the root of the repository
    $ wget https://fedorapeople.org/groups/risc-v/disk-images/vmlinux
    $ mv vmlinux fedora-vmlinux
    $ wget https://fedorapeople.org/groups/risc-v/disk-images/stage4-disk.img.xz
    $ unxz stage4-disk.img.xz


Instead of that disk image, you can also use a more recent one from [here](http://185.97.32.145/koji/tasks?state=closed&view=flat&method=createAppliance&order=-id) (Some links there have invalid TLS certs, replace 'https://fedora-riscv.tranquillity.se' with the IP address version 'http://185.97.32.145'). If you do, you may have to replace the disk image name or kernel boot arguments to select the right boot partition.

### Configure COW disk for the guest

If you want to avoid accidentally corrupting the your base disk image, you can use a copy-on-write disk instead:

    $ chmod -w Fedora-Developer-Rawhide-20190506.n.0-sda
	$ qemu-img create -f qcow2 -b Fedora-Developer-Rawhide-20190506.n.0-sda.raw -F raw img01.qcow2

## Instructions

Download RVirt's source code:

    $ git clone https://github.com/fintelia/rvirt
    $ cd rvirt

From inside the repository root directory, install the Rust RISC-V target (RVirt pins a specific compiler version so rustup needs to see its 'rust-toolchain' file):

    $ rustup target add riscv64imac-unknown-none-elf

Make any necessary edits to Makefile

    - If your kernel image isn't named 'fedora-vmlinux' or your disk 'stage4-disk.img' then you'll want to change the appropriate line.
	- If you want to pass different arguments to Linux (say because the root directory of your disk image is /dev/vda1 instead of /dev/vda) edit the -append "..." line accordingly.

Build and run RVirt:

    $ make qemu

Once boot is complete (which can take 4-5 minutes) you can SSH into the guest machine. The root password is likely to be 'riscv':

    $ ssh -p 10001 root@localhost

If you want to debug using gdb, run these commands in the project directory in separate shells:

    $ make qemu-gdb
    $ riscv64-unknown-elf-gdb

## Current Status

RVirt supports running both inside an emulator and on real hardware and does runtime detection to learn what platform it is executing on. It has so far been tested with Fedora RISC-V builds, but likely  at least partially supports other distributions as well.

### Supported Platforms

Tier 1: Boots fully and supports interaction via SSH / serial console

* QEMU virt machine type

Tier 2: Boots partially but lacks driver support for block/network device to complete boot process

* HiFive Unleashed board
* QEMU sifiveu machine type

### Correctness

- [x] Trap and emulate of privileged instructions (CSR related and SFENCE.VMA)
- [x] Shadow page tables
- [x] Update PTE accessed and dirty bits
- [x] Timers
- [x] Expose and/or emulate peripherals
- [x] Address lingering bugs in boot process

### Functionality
Additional features not needed for the correct virtualization of a single guest:

- [x] multiple guests
- [x] passthrough of virtio block and network devices
- [ ] paravirtualized network devices backed by HiFive Unleashed's NIC
- [ ] multicore guests and inter-processor interrupts between them

Other features not used by Linux / not supported by current platforms are unlikely to be implemented:

- [ ] ASID support
- [ ] Sv48 or Sv57 guest page tables (only Sv39 currently allowed)
- [ ] SR-IOV PCIe devices
- [ ] 32-bit guests