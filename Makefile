all: src/*.rs Cargo.toml
	xargo rustc --target riscv32imac-unknown-none-elf -- -C link-arg=-Tsrc/linker.ld  -C linker=riscv32-unknown-elf-ld -Z linker-flavor=ld

svpn: *.c Makefile
	riscv64-unknown-elf-gcc -O0 -nostdlib *.c -o svpn

