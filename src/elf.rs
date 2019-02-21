#![allow(unused)]

// Values for ProgramHeader::type_
const ELF_PROG_LOAD: u32 = 1;

// Flag bits for ProgramHeader::flags
const ELF_PROG_FLAG_EXEC: u32 = 1;
const ELF_PROG_FLAG_WRITE: u32 = 2;
const ELF_PROG_FLAG_READ: u32 = 4;

// Values for SectionHeader::type_
const ELF_SHT_NULL: u32 = 0;
const ELF_SHT_PROGBITS: u32 = 1;
const ELF_SHT_SYMTAB: u32 = 2;
const ELF_SHT_STRTAB: u32 = 3;

// Values for SectionHeader::name
const ELF_SHN_UNDEF: u32 = 0;

const ELF_MAGIC: u32 = 0;

#[repr(C)]
#[derive(Debug)]
pub struct Ident {
    magic: u32,
    class: u8,
    data: u8,
    version: u8,
    osabi: u8,
    abiversion: u8,
    padding: [u8; 7],
}

#[repr(C)]
#[derive(Debug)]
pub struct Elf64 {
    ident: Ident,
    type_: u16,
	machine: u16,
	version: u32,
	entry: u64,
	phoff: u64,
	shoff: u64,
	flags: u32,
	ehsize: u16,
	phentsize: u16,
	phnum: u16,
	shentsize: u16,
	shnum: u16,
	shstrndx: u16,
}

#[repr(C)]
#[derive(Debug)]
pub struct ProgramHeader64 {
    type_: u32,
    flags: u32,
    offset: u64,
    va: u64,
    pa: u64,
    file_size: u64,
    memory_size: u64,
    align: u64,
}

// Returns program entry point
pub unsafe fn load_elf(data: *const u8, base_address: *mut u8) -> *const u8 {
    let elf = &*(data as *const Elf64);
    assert_eq!(elf.ident.magic, 0x464C457F);
    assert_eq!(elf.ident.class, 2); // 64-bit
    assert_eq!(elf.ident.data, 1); // Little endian
    assert_eq!(elf.machine, 243); // Machine = RISCV
    assert_eq!(elf.type_, 2); // 64-bit
    assert_eq!(elf.version, 1);

    for i in 0..(elf.phnum as usize) {
        let ph = &*(data.add(elf.phoff as usize + i * elf.phentsize as usize) as *const ProgramHeader64);

        if ph.type_ == ELF_PROG_LOAD {
            if ph.file_size > 0 {
                let dst = base_address.add(ph.pa as usize);
                let src = data.add(ph.offset as usize);
                core::ptr::copy(src, dst, ph.file_size as usize);
            }
            if ph.memory_size > ph.file_size {
                let dst = base_address.add((ph.pa + ph.file_size) as usize);
                core::ptr::write_bytes(dst, 0, (ph.memory_size - ph.file_size) as usize);
            }
        }
    }

    //    base_address.add(elf.entry as usize)
    0x80000000 as *const u8
}
