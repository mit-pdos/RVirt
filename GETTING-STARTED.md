# Getting Started

## Installing dependencies

 - rustup: https://rustup.rs/
   - customize configuration to select the "nightly" build during setup.

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

    $ git clone https://github.com/mit-pdos/rvirt && cd rvirt

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