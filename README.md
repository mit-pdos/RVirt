# RVirt
![Travis](https://img.shields.io/travis/mit-pdos/rvirt.svg)
![License](https://img.shields.io/github/license/mit-pdos/rvirt.svg)
![Language](https://img.shields.io/github/languages/top/mit-pdos/rvirt.svg)

RVirt is an S-mode trap-and-emulate hypervisor for RISC-V. It is currently targeted at QEMU's virt machine type, but partially supports the HiFive Unleashed as well. It can run either on either the [Berkley Boot Loader](https://github.com/riscv/riscv-pk) or with its own (considerably faster) M-mode stub. It is powerful enough to run multiple instances of Linux as guest operating systems.

## FAQ

### How is RVirt different from other hypervisors like [Firecracker](https://github.com/firecracker-microvm/firecracker), [Cloud Hypervisor](https://github.com/intel/cloud-hypervisor) or [xvisor](https://github.com/avpatel/xvisor-next)?

All three of the other projects can only run on processors that have hardware virtualization extensions like Intel VT-x or RISC-V's planned H-extension. Firecracker and Cloud additionally depend on KVM (and by extension the entire Linux kernel). By contrast, RVirt doesn't need KVM or Linux and can run on any sufficiently powerful 64-bit RISC-V processor with an MMU.

### Why RISC-V?

RISC-V is [classically virtualizable](https://en.wikipedia.org/wiki/Popek_and_Goldberg_virtualization_requirements) which means that a hypervisor can rely on any privileged instruction triggering an illegal instruction fault when executed by the (unprivileged) guest OS. This is in contrast to other ISAs like x86 which have instructions that behave differently in user and kernel mode but never trap. Additionally, RISC-V has only 12 supervisor level control registers and only a handful of privileged instructions making the work to implement trap and emulate much more manageable.

### Why Rust?

Why not? Rust is a pleasant language to work with and can directly target bare metal platforms. I was also excited by Rust's ability to guarantee memory safety for safe code, but I found the amount of unsafe code required for initialization and vm entry/exit partially negated this benefit.

## Getting Started

For more detailed instructions, see the [getting started guide](GETTING-STARTED.md).

### Install Dependencies

    $ curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
    $ sudo apt-get install qemu-system-misc

### Clone the repository

    $ git clone https://github.com/mit-pdos/rvirt && cd rvirt

### Download guest images

    $ wget https://fedorapeople.org/groups/risc-v/disk-images/vmlinux
    $ mv vmlinux fedora-vmlinux
    $ wget https://fedorapeople.org/groups/risc-v/disk-images/stage4-disk.img.xz
    $ unxz stage4-disk.img.xz

### Compile and run

    $ make qemu

This command launches an instance of QEMU with RVirt running inside and Linux running inside that. Once the boot process has completed you can SSH in through all the layers and directly interact with the guest (root password is 'riscv'):

    $ ssh -p 10001 root@localhost

## Current Status

RVirt supports running both inside an emulator and on real hardware and does runtime detection to learn what platform it is executing on. It has so far been tested with Fedora RISC-V builds, but may work with other distributions as well.

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
In addition to being able to boot and run a single guest, RVirt also supports some features not needed for the correct virtualization of a single guest:

- [x] multiple guests
- [x] passthrough of virtio block and network devices
- [ ] paravirtualized network devices backed by HiFive Unleashed's NIC *(in progress)*
- [ ] multicore guests and inter-processor interrupts between them

Other features not used by Linux / not supported by current platforms are unlikely to be implemented:

- [ ] ASID support
- [ ] Sv48 or Sv57 guest page tables (only Sv39 currently allowed)
- [ ] SR-IOV PCIe devices
- [ ] 32-bit guests


