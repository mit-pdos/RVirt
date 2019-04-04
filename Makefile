release: src/*.rs Cargo.toml src/linker.ld
	cargo rustc --release --target riscv64imac-unknown-none-elf -- -C link-arg=-Tsrc/linker.ld  -C linker=riscv64-unknown-elf-ld

qemu: release
	qemu-system-riscv64 -machine virt -kernel target/riscv64imac-unknown-none-elf/release/rvirt -nographic -initrd fedora-vmlinux -m 2G -smp 2 \
	    -object rng-random,filename=/dev/urandom,id=rng0 \
	    -device virtio-rng-device,rng=rng0 \
	    -append "console=ttyS0 ro root=/dev/vda" \
	    -device virtio-blk-device,drive=hd0 \
	    -drive file=stage4-disk.img,format=raw,id=hd0 \
	    -device virtio-net-device,netdev=usernet \
	    -netdev user,id=usernet,hostfwd=tcp::10000-:22
