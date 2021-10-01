use std::path::PathBuf;
use std::process::Command;

const CC: &'static str = "gcc";
const LD: &'static str = "ld";
const OBJCOPY: &'static str = "objcopy";

const CFLAGS: &[&'static str] = &[
    "-fno-pic",
    "-static",
    "-fno-builtin",
    "-fno-strict-aliasing",
    "-O2",
    "-Wall",
    //"-MD",
    //"-ggdb",
    "-m32",
    //"-Werror",
    "-fno-omit-frame-pointer",
    "-fno-stack-protector",
];

const LDFLAGS: &[&'static str] = &["-m", "elf_i386"];

fn main() {
    let mut build = cc::Build::new();
    {
        build.include("c/");
        for flag in CFLAGS {
            build.flag(flag);
        }
    }

    let c_files = [
        "bio",
        "console",
        "exec",
        "file",
        "fs",
        "ide",
        "kalloc",
        "kbd",
        "log",
        "main",
        "mp",
        "pipe",
        "proc",
        "sleeplock",
        "spinlock",
        "string",
        "syscall",
        "sysfile",
        "sysproc",
        "trap",
        "vm",
    ];
    for file in c_files {
        build
            .clone()
            .file(&format!("c/{}.c", file))
            .compile(&format!("lib{}.a", file));
        println!("cargo:rustc-link-lib=static={}", file);
    }

    let asm_files = ["entry"];
    for file in asm_files {
        build
            .clone()
            .file(&format!("c/{}.S", file))
            .compile(&format!("lib{}.a", file));
        println!("cargo:rustc-link-lib=static={}", file);
    }

    let out_path = PathBuf::from(std::env::var("OUT_DIR").unwrap());
    initcode(&out_path);
    entryother(&out_path);
}

fn initcode(out_path: &PathBuf) {
    Command::new(CC)
        .args(CFLAGS)
        .args(&["-nostdinc", "-I.", "-c", "c/initcode.S", "-o"])
        .arg(out_path.join("initcode.o"))
        .status()
        .unwrap();

    Command::new(LD)
        .args(LDFLAGS)
        .args(&["-N", "-e", "start", "-Ttext", "0", "-o"])
        .arg(out_path.join("initcode.out"))
        .arg(out_path.join("initcode.o"))
        .status()
        .unwrap();

    Command::new(OBJCOPY)
        .args(&["-S", "-O", "binary"])
        .arg(out_path.join("initcode.out"))
        .arg(out_path.join("initcode"))
        .status()
        .unwrap();
}

fn entryother(out_path: &PathBuf) {
    Command::new(CC)
        .args(CFLAGS)
        .args(&["-fno-pic", "-nostdinc", "-I.", "-c", "c/entryother.S", "-o"])
        .arg(out_path.join("entryother.o"))
        .status()
        .unwrap();

    Command::new(LD)
        .args(LDFLAGS)
        .args(&["-N", "-e", "start", "-Ttext", "0x7000", "-o"])
        .arg(out_path.join("entryother.out"))
        .arg(out_path.join("entryother.o"))
        .status()
        .unwrap();

    Command::new(OBJCOPY)
        .args(&["-S", "-O", "binary", "-j", ".text"])
        .arg(out_path.join("entryother.out"))
        .arg(out_path.join("entryother"))
        .status()
        .unwrap();
}
