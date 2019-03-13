debug: src/*.rs Cargo.toml src/linker.ld
	cargo rustc --target riscv64imac-unknown-none-elf -- -C link-arg=-Tsrc/linker.ld  -C linker=riscv64-unknown-elf-ld

release: src/*.rs Cargo.toml src/linker.ld
	cargo rustc --release --target riscv64imac-unknown-none-elf -- -C link-arg=-Tsrc/linker.ld  -C linker=riscv64-unknown-elf-ld

svpn: *.c Makefile
	riscv32-unknown-elf-gcc -O0 -nostdlib *.c -o svpn

qemu: debug
	/home/jonathan/git/qemu/build/riscv64-softmmu/qemu-system-riscv64 -machine virt -kernel target/riscv64imac-unknown-none-elf/debug/svpn -nographic -initrd fedora-vmlinux -m 2G \
	    -object rng-random,filename=/dev/urandom,id=rng0 \
	    -device virtio-rng-device,rng=rng0 \
	    -append "console=ttyS0 ro root=/dev/vda" \
	    -device virtio-blk-device,drive=hd0 \
	    -drive file=stage4-disk.img,format=raw,id=hd0

qemu-release: release
	qemu-system-riscv64 -machine virt -kernel target/riscv64imac-unknown-none-elf/release/svpn -nographic -initrd fedora-vmlinux -m 2G \
	    -object rng-random,filename=/dev/urandom,id=rng0 \
	    -device virtio-rng-device,rng=rng0 \
	    -append "console=ttyS0 ro root=/dev/vda" \
	    -device virtio-blk-device,drive=hd0 \
	    -drive file=stage4-disk.img,format=raw,id=hd0
