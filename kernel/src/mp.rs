use core::ffi::c_void;

use crate::memlayout::p2v;

// See MultiProcessor Specification Version 1.[14]

// Table entry types
pub const MPPROC: usize = 0x00; // One per processor
pub const MPBUS: usize = 0x01; // One per bus
pub const MPIOAPIC: usize = 0x02; // One per I/O APIC
pub const MPIOINTR: usize = 0x03; // One per bus interrupt source
pub const MPLINTR: usize = 0x04; // One per system interrupt source

// floating pointer
#[repr(C)]
pub struct MP {
    signature: [u8; 4],      // "_MP_"
    physaddr: *const c_void, // phys addr of MP config table
    length: u8,              // 1
    specrev: u8,             // [14]
    checksum: u8,            // all bytes must add up to 0
    ty: u8,                  // MP system config type
    imcrp: u8,
    reserved: [u8; 3],
}

impl MP {
    pub fn is_valid(&self) -> bool {
        const SIGN: [u8; 4] = ['_' as u8, 'M' as u8, 'P' as u8, '_' as u8];
        self.signature == SIGN && sum(self) == 0
    }
}

// configuration table header
#[repr(C)]
pub struct MPConf {
    signature: [u8; 4],    // "PCMP"
    length: u16,           // total table length
    version: u8,           // [14]
    checksum: u8,          // all bytes must add up to 0
    product: [u8; 20],     // product id
    oemtable: *const u32,  // OEM table pointer
    oemlength: u16,        // OEM table length
    entry: u16,            // entry count
    lapicaddr: *const u32, // address of local APIC
    xlength: u16,          // extended table length
    xchecksum: u8,         // extended table checksum
    reserved: u8,
}

impl MPConf {
    pub fn is_valid(&self) -> bool {
        const SIGN: [u8; 4] = ['P' as u8, 'C' as u8, 'M' as u8, 'P' as u8];
        self.signature == SIGN
            && (self.version == 1 || self.version == 4)
            && sum_by_length(self as *const _ as usize, self.length as usize) == 0
    }
}

// processor table entry
pub struct MPProc {
    ty: u8,             // entry type (0)
    apicid: u8,         // local APIC id
    version: u8,        // local APIC verison
    flags: u8,          // CPU flags
    signature: [u8; 4], // CPU signature
    feature: u32,       // feature flags from CPUID instruction
    reserved: [u8; 8],
}

impl MPProc {
    pub const MPBOOT: usize = 0x02; // This proc is the bootstrap processor.
}

// I/O APIC table entry
pub struct MPIOApic {
    ty: u8,           // entry type (2)
    apicno: u8,       // I/O APIC id
    version: u8,      // I/O APIC version
    flags: u8,        // I/O APIC flags
    addr: *const u32, // I/O APIC address
}

fn sum_by_length(addr: usize, len: usize) -> u8 {
    let ptr = addr as *const u8;
    let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
    bytes
        .into_iter()
        .fold(0u8, |acc, byte| acc.wrapping_add(*byte))
}

fn sum<T>(v: &T) -> u8 {
    sum_by_length(v as *const _ as usize, core::mem::size_of::<T>())
}

// Search for the MP Floating Pointer Structure, which according to the
// spec is in one of the following three locations:
// 1) in the first KB of the EBDA;
// 2) in the last KB of system base memory;
// 3) in the BIOS ROM between 0xE0000 and 0xFFFFF.
fn mp_search() -> Option<&'static MP> {
    // Look for an MP structure in the len bytes at addr.
    fn search(addr: usize, len: usize) -> Option<&'static MP> {
        let slice = unsafe {
            core::slice::from_raw_parts(p2v(addr) as *const MP, len / core::mem::size_of::<MP>())
        };

        slice.iter().find(|mp| mp.is_valid())
    }

    unsafe {
        let bda = p2v(0x400) as *const u8;
        let ptr = ((*bda.offset(0x0f) as usize) << 8 | *bda.offset(0x0e) as usize) << 4;

        if ptr != 0 {
            if let Some(mp) = search(ptr, 1024) {
                return Some(mp);
            }
        } else {
            let ptr = ((*bda.add(0x14) as usize) << 8 | (*bda.add(0x13) as usize)) * 1024;
            if let Some(mp) = search(ptr - 1024, 1024) {
                return Some(mp);
            }
        }

        search(0xf0000, 0x10000)
    }
}

// Search for an MP configuration table.  For now,
// don't accept the default configurations (physaddr == 0).
// Check for correct signature, calculate the checksum and,
// if correct, check the version.
// To do: check extended table checksum.
fn mp_config() -> Option<(&'static MP, &'static MPConf)> {
    let mp = mp_search().filter(|mp| !mp.physaddr.is_null())?;
    let conf = p2v(mp.physaddr as usize);
    let conf = conf as *const MPConf;

    unsafe {
        match (*conf).is_valid() {
            true => Some((mp, &(*conf))),
            false => None,
        }
    }
}

mod _binding {
    use super::*;

    #[no_mangle]
    extern "C" fn mpsearch() -> *const MP {
        match mp_search() {
            Some(ptr) => ptr as *const MP,
            None => core::ptr::null(),
        }
    }

    #[no_mangle]
    extern "C" fn mpconfig(pmp: *mut *const MP) -> *const MPConf {
        match mp_config() {
            Some((mp, conf)) => unsafe {
                *pmp = mp;
                conf
            },
            None => core::ptr::null(),
        }
    }
}
