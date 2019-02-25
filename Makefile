debug: src/*.rs Cargo.toml src/linker.ld
	cargo rustc --target riscv64imac-unknown-none-elf -- -C link-arg=-Tsrc/linker.ld  -C linker=riscv64-unknown-elf-ld

release: src/*.rs Cargo.toml src/linker.ld
	cargo rustc --release --target riscv64imac-unknown-none-elf -- -C link-arg=-Tsrc/linker.ld  -C linker=riscv64-unknown-elf-ld

svpn: *.c Makefile
	riscv32-unknown-elf-gcc -O0 -nostdlib *.c -o svpn

qemu: debug
	qemu-system-riscv64 -machine virt -kernel target/riscv64imac-unknown-none-elf/debug/svpn -nographic -d guest_errors -initrd fedora-vmlinux -m 1G -append "console=ttyS0 ro root=/dev/vda"
