fn main() {
    println!(
        "cargo:rustc-link-search={}",
        std::env::var("OUT_DIR").unwrap()
    );

    let mut build = cc::Build::new();
    {
        build
            //.warnings(true)
            .flag("-fno-pic")
            .flag("-fno-pie")
            .flag("-no-pie")
            .flag("-static")
            .flag("-fno-builtin")
            .flag("-fno-strict-aliasing")
            .flag("-O2")
            .flag("-Wall")
            //.flag("-MD")
            .flag("-ggdb")
            .flag("-m32")
            //.flag("-Werror")
            .flag("-fno-omit-frame-pointer")
            .flag("-fno-stack-protector")
            .include("c/");
    }

    let c_files = [
        "bio",
        "console",
        "exec",
        "file",
        "fs",
        "ide",
        "ioapic",
        "kalloc",
        "kbd",
        "lapic",
        "log",
        "main",
        "mp",
        "picirq",
        "pipe",
        "proc",
        "sleeplock",
        "spinlock",
        "string",
        "syscall",
        "sysfile",
        "sysproc",
        "trap",
        "uart",
        "vm",
    ];
    for file in c_files {
        build
            .clone()
            .file(&format!("c/{}.c", file))
            .compile(&format!("lib{}.a", file));
        println!("cargo:rustc-link-lib=static={}", file);
    }

    let asm_files = ["swtch", "trapasm", "vectors"];
    for file in asm_files {
        build
            .clone()
            .file(&format!("c/{}.S", file))
            .compile(&format!("lib{}.a", file));
        println!("cargo:rustc-link-lib=static={}", file);
    }
}
