debug: src/*.rs Cargo.toml src/linker.ld
	cargo rustc --target riscv64imac-unknown-none-elf -- -C link-arg=-Tsrc/linker.ld  -C linker=riscv64-unknown-elf-ld

release: src/*.rs Cargo.toml src/linker.ld
	cargo rustc --release --target riscv64imac-unknown-none-elf -- -C link-arg=-Tsrc/linker.ld  -C linker=riscv64-unknown-elf-ld

svpn: *.c Makefile
	riscv32-unknown-elf-gcc -O0 -nostdlib *.c -o svpn

qemu: debug
	qemu-system-riscv64 -machine virt -kernel target/riscv64imac-unknown-none-elf/debug/svpn -nographic -d guest_errors

qemu-gdb: debug
	qemu-system-riscv32 -s -S -smp 2 -machine virt -kernel target/riscv32imac-unknown-none-elf/debug/svpn -nographic

qemux: debug
	/home/jonathan/git/qemu/build/riscv32-softmmu/qemu-system-riscv32 -machine virt -kernel target/riscv32imac-unknown-none-elf/debug/svpn -nographic -initrd /home/jonathan/Downloads/fedora-vmlinux -m 2048
