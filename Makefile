OUT=target/riscv64imac-unknown-none-elf/release

################################################################################
#                               COMPILE BINARIES                               #
################################################################################

GUEST_KERNEL_FEATURE=$(if $(RVIRT_GUEST_KERNEL), --features embed_guest_kernel, )

# Build the main rvirt binary. Relies on an SBI inteface for some functionality.
$(OUT)/rvirt: src/*.rs src/*/*.rs src/*.S Cargo.toml src/slinker.ld rustup-target
	cargo rustc --release --target riscv64imac-unknown-none-elf --bin rvirt \
	    $(GUEST_KERNEL_FEATURE) -- -C link-arg=-Tsrc/slinker.ld

# Flattened version of rvirt binary.
$(OUT)/rvirt.bin: $(OUT)/rvirt
	objcopy -S -I elf64-little -O binary --change-addresses -0x80000000 \
	    --set-section-flags .bss=alloc,load,contents \
	    $(OUT)/rvirt $(OUT)/rvirt.bin

# Build a free standing binary that can run directly on bare metal without any
# SBI provider.
$(OUT)/rvirt-bare-metal: $(OUT)/rvirt.bin src/*.rs src/*/*.rs src/*.S Cargo.toml src/mlinker.ld rustup-target
	PAYLOAD=$(OUT)/rvirt.bin cargo rustc --release --target \
	    riscv64imac-unknown-none-elf --bin rvirt-bare-metal --features \
	    "physical_symbol_addresses" -- -C link-arg=-Tsrc/mlinker.ld

# Flattened version of rvirt-bare-metal binary.
$(OUT)/rvirt-bare-metal.bin: $(OUT)/rvirt-bare-metal
	objcopy -S -I elf64-little -O binary --change-addresses -0x80000000 \
	    $(OUT)/rvirt-bare-metal $(OUT)/rvirt-bare-metal.bin

################################################################################
#                              QEMU RUN COMMANDS                               #
################################################################################

# Run rvirt inside QEMU.
qemu: $(OUT)/rvirt-bare-metal
	qemu-system-riscv64 -machine virt -nographic -m 2G -smp 1 $(GDBOPTS) \
	    -kernel $(OUT)/rvirt-bare-metal -initrd fedora-vmlinux \
	    -append "console=ttyS0 ro root=/dev/vda" \
	    -object rng-random,filename=/dev/urandom,id=rng1 \
	    -device virtio-rng-device,rng=rng1,bus=virtio-mmio-bus.0 \
	    -device virtio-blk-device,drive=hd1,bus=virtio-mmio-bus.1 \
	    -drive file=stage4-disk.img,format=raw,id=hd1 \
	    -device virtio-net-device,netdev=usernet1,bus=virtio-mmio-bus.2 \
	    -netdev user,id=usernet1,hostfwd=tcp::10001-:22

# Run rvirt inside QEMU with BBL as the SBI provider. Requires a build of QEMU
# with support for the `-bios` flag which mainline QEMU doesn't yet have.
qemu-bbl: $(OUT)/rvirt.bin
	qemu-system-riscv64 -machine virt -nographic -m 2G -smp 1 \
	    -bios bbl -kernel $(OUT)/rvirt.bin -initrd fedora-vmlinux \
	    -append "console=ttyS0 root=/dev/vda2" \
	    -object rng-random,filename=/dev/urandom,id=rng1 \
	    -device virtio-rng-device,rng=rng1,bus=virtio-mmio-bus.0 \
	    -device virtio-blk-device,drive=hd1,bus=virtio-mmio-bus.1 \
	    -drive file=img01.qcow2,format=qcow2,id=hd1 \
	    -device virtio-net-device,netdev=usernet1,bus=virtio-mmio-bus.2 \
	    -netdev user,id=usernet1,hostfwd=tcp::10001-:22

# Run rvirt inside QEMU but target the sifive_u machine type.
qemu-sifive: $(OUT)/rvirt-bare-metal
	~/git/qemu/build/riscv64-softmmu/qemu-system-riscv64 -machine sifive_u -nographic -m 2G \
	    -append "raid=noautodetect nfsrootdebug earlyprintk ip=::::riscv root=/dev/nfs rw nfsroot=/srv/nfs4/stage4,port=2049,vers=3 ipv6.disable=1" \
	    -kernel $(OUT)/rvirt-bare-metal -nic user,id=net0 \
	    -object filter-dump,id=net0,netdev=net0,file=dump.dat



# -netdev user,id=net0,hostfwd=tcp::10000-:10000



#        -netdev user,id=net0


# Run rvirt inside QEMU but wait for GDB to attach on port 26000 first.
GDBOPTS=$(if $(DEBUG),-gdb tcp::26000 -S,)
qemu-gdb: DEBUG=1
qemu-gdb: qemu

################################################################################
#                          HIFIVE UNLEASHED COMMANDS                           #
################################################################################

# Prepare a `.fit` file and place it in /srv/tftp so the HiFive Unleashed can
# boot from it. Requires atftpd with target directory set to /srv/tftp/.
fit: $(OUT)/rvirt-bare-metal.bin uboot-fit-image.its
	mkimage -f uboot-fit-image.its -A riscv -O linux -T flat_dt $(OUT)/rvirt.fit
	cp $(OUT)/rvirt.fit /srv/tftp/hifiveu.fit

# Display serial output from the HiFive Unleashed. To get line endings to be
# correct, follow steps described on:
# https://unix.stackexchange.com/questions/283924/how-can-minicom-permanently-translate-incoming-newline-n-to-crlf
serial-output:
	sudo minicom -D /dev/serial/by-id/usb-FTDI_Dual_RS232-HS-if01-port0

################################################################################
#                                MISC COMMANDS                                 #
################################################################################

rustup-target:
	rustup target add riscv64imac-unknown-none-elf || true
