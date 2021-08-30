OUTPUT_FORMAT("elf32-i386")
OUTPUT_ARCH(i386)
ENTRY(start)
MBRBASE = 0x7C00;

SECTIONS {
    . = MBRBASE;
    .text : {
      *(.text)
      *(.text.*)
    }
	 .rodata : {
      . = ALIGN(4);
      *(.rodata)
      *(.rodata.*)
	 }

    /* .textが510バイトを超えているとブートシグニチャが入らなくてエラー */

    . = MBRBASE + 510;
    .sign : { SHORT(0xAA55) } /* ブートシグニチャ */
}