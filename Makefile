all: src/*.rs Cargo.toml
	xargo rustc --target riscv32imac-unknown-none-elf -- -C link-arg=-Tsrc/linker.ld  -C linker=riscv32-unknown-elf-ld -Z linker-flavor=ld

svpn: *.c Makefile
	riscv64-unknown-elf-gcc -O0 -nostdlib *.c -o svpn

qemu: all
	qemu-system-riscv32 -machine virt -kernel target/riscv32imac-unknown-none-elf/debug/svpn -nographic

qemu-sifive: all
	qemu-system-riscv32 -s -S -d guest_errors -machine sifive_u -kernel target/riscv32imac-unknown-none-elf/debug/svpn -nographic
