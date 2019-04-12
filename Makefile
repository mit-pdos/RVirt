release: src/*.rs Cargo.toml src/linker.ld
	cargo rustc --release --target riscv64imac-unknown-none-elf -- -C link-arg=-Tsrc/linker.ld  -C linker=riscv64-unknown-elf-ld

# note: this maps rng -> virtio2, blk -> virtio1, net -> virtio0. see virtio-order.md for explanation.
qemu: release
	qemu-system-riscv64 -machine virt -kernel target/riscv64imac-unknown-none-elf/release/rvirt -nographic -initrd fedora-vmlinux -m 3G -smp 3 \
	    -append "console=ttyS0 ro root=/dev/vda" \
	    -object rng-random,filename=/dev/urandom,id=rng0 \
	    -device virtio-rng-device,rng=rng0,bus=virtio-mmio-bus.4 \
	    -device virtio-blk-device,drive=hd0,bus=virtio-mmio-bus.5 \
	    -drive file=stage4-disk.img.2,format=raw,id=hd0 \
	    -device virtio-net-device,netdev=usernet,bus=virtio-mmio-bus.6 \
	    -netdev user,id=usernet,hostfwd=tcp::10000-:22
	    -object rng-random,filename=/dev/urandom,id=rng1 \
	    -device virtio-rng-device,rng=rng1,bus=virtio-mmio-bus.0 \
	    -device virtio-blk-device,drive=hd1,bus=virtio-mmio-bus.1 \
	    -drive file=stage4-disk.img,format=raw,id=hd1 \
	    -device virtio-net-device,netdev=usernet2,bus=virtio-mmio-bus.2 \
	    -netdev user,id=usernet2,hostfwd=tcp::10001-:22

qemu-sifive: release
	qemu-system-riscv64 -machine sifive_u -kernel target/riscv64imac-unknown-none-elf/release/rvirt -nographic -initrd fedora-vmlinux -m 2G

