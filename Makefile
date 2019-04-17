release: src/*.rs Cargo.toml src/linker.ld
	cargo rustc --release --target riscv64imac-unknown-none-elf -- -C link-arg=-Tsrc/linker.ld  -C linker=riscv64-unknown-elf-ld

GDBOPTS=$(if $(DEBUG),-gdb tcp::26000 -S,)

qemu: release
	# note: this maps rng -> virtio2, blk -> virtio1, net -> virtio0. see virtio-order.md for explanation.
	qemu-system-riscv64 -machine virt -kernel target/riscv64imac-unknown-none-elf/release/rvirt -nographic -initrd fedora-vmlinux -m 2G -smp 2 $(GDBOPTS) \
	    -object rng-random,filename=/dev/urandom,id=rng0 \
	    -device virtio-rng-device,rng=rng0,bus=virtio-mmio-bus.0 \
	    -append "console=ttyS0 ro root=/dev/vda" \
	    -device virtio-blk-device,drive=hd0,bus=virtio-mmio-bus.1 \
	    -drive file=stage4-disk.img,format=raw,id=hd0 \
	    -device virtio-net-device,netdev=usernet,bus=virtio-mmio-bus.2 \
	    -netdev user,id=usernet,hostfwd=tcp::10000-:22

# to debug, run make qemu-gdb, and then run gdb
qemu-gdb: DEBUG=1
qemu-gdb: qemu
