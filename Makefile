LD=riscv64-unknown-elf-ld

GUEST_KERNEL_FEATURE=$(if $(RVIRT_GUEST_KERNEL), --features embed_guest_kernel, )

release: src/*.rs src/*.S Cargo.toml src/linker.ld
	cargo rustc --release --target riscv64imac-unknown-none-elf --bin rvirt-supervisor $(GUEST_KERNEL_FEATURE) -- -C link-arg=-Tsrc/slinker.ld  -C linker=$(LD)
	cargo rustc --release --target riscv64imac-unknown-none-elf --bin rvirt-machine --features "physical_symbol_addresses" -- -C link-arg=-Tsrc/mlinker.ld  -C linker=$(LD)
	$(LD) -Tsrc/linker.ld target/riscv64imac-unknown-none-elf/release/rvirt-supervisor target/riscv64imac-unknown-none-elf/release/rvirt-machine -o target/riscv64imac-unknown-none-elf/release/rvirt

binary: release
	objcopy -S -O binary --change-addresses -0x80000000 --set-section-flags .bss=alloc,load,contents target/riscv64imac-unknown-none-elf/release/rvirt target/riscv64imac-unknown-none-elf/release/rvirt.bin

# Requires atftpd with target directory set to /tftpboot
fit: binary uboot-fit-image.its
	mkimage -f uboot-fit-image.its -A riscv -O linux -T flat_dt target/riscv64imac-unknown-none-elf/release/rvirt.fit
	cp target/riscv64imac-unknown-none-elf/release/rvirt.fit /srv/tftp/hifiveu.fit

# note: this maps rng -> virtio2, blk -> virtio1, net -> virtio0. see virtio-order.md for explanation.
qemu: release
	qemu-system-riscv64 -machine virt -kernel target/riscv64imac-unknown-none-elf/release/rvirt -nographic -initrd fedora-vmlinux -m 3G -smp 3 $(GDBOPTS) \
	    -append "console=ttyS0 ro root=/dev/vda" \
	    -object rng-random,filename=/dev/urandom,id=rng0 \
	    -device virtio-rng-device,rng=rng0,bus=virtio-mmio-bus.4 \
	    -device virtio-blk-device,drive=hd0,bus=virtio-mmio-bus.5 \
	    -drive file=stage4-disk.img.2,format=raw,id=hd0 \
	    -device virtio-net-device,netdev=usernet0,bus=virtio-mmio-bus.6 \
	    -netdev user,id=usernet0,hostfwd=tcp::10000-:22 \
	    -object rng-random,filename=/dev/urandom,id=rng1 \
	    -device virtio-rng-device,rng=rng1,bus=virtio-mmio-bus.0 \
	    -device virtio-blk-device,drive=hd1,bus=virtio-mmio-bus.1 \
	    -drive file=stage4-disk.img,format=raw,id=hd1 \
	    -device virtio-net-device,netdev=usernet1,bus=virtio-mmio-bus.2 \
	    -netdev user,id=usernet1,hostfwd=tcp::10001-:22

qemu-sifive: release
	qemu-system-riscv64 -machine sifive_u -kernel target/riscv64imac-unknown-none-elf/release/rvirt -nographic -initrd fedora-vmlinux -m 2G

GDBOPTS=$(if $(DEBUG),-gdb tcp::26000 -S,)

# to debug, run make qemu-gdb, and then run gdb
qemu-gdb: DEBUG=1
qemu-gdb: qemu

# To get line endings to be correct, follow steps described on:
# https://unix.stackexchange.com/questions/283924/how-can-minicom-permanently-translate-incoming-newline-n-to-crlf
serial-output:
	sudo minicom -D /dev/serial/by-id/usb-FTDI_Dual_RS232-HS-if01-port0
