debug: src/*.rs Cargo.toml
	xargo rustc --target riscv32imac-unknown-none-elf -- -C link-arg=-Tsrc/linker.ld  -C linker=riscv32-unknown-elf-ld #-Z linker-flavor=ld

release: src/*.rs Cargo.toml
	xargo rustc --release --target riscv32imac-unknown-none-elf -- -C link-arg=-Tsrc/linker.ld  -C linker=riscv32-unknown-elf-ld -Z linker-flavor=ld

svpn: *.c Makefile
	riscv64-unknown-elf-gcc -O0 -nostdlib *.c -o svpn

qemu: debug
	qemu-system-riscv32 -machine virt -kernel target/riscv32imac-unknown-none-elf/debug/svpn -nographic

qemu-gdb: debug
	qemu-system-riscv32 -s -S -smp 2 -machine virt -kernel target/riscv32imac-unknown-none-elf/debug/svpn -nographic
