use crate::{console::consoleintr, x86::inb};

// PC keyboard interface constants

const KBSTATP: u16 = 0x64; // kbd controller status port(I)
const KBS_DIB: u8 = 0x01; // kbd data in buffer
const KBDATAP: u16 = 0x60; // kbd data port(I)

const NO: u8 = 0;
const BS: u8 = 0x08;

const SHIFT: u8 = 1 << 0;
const CTL: u8 = 1 << 1;
const ALT: u8 = 1 << 2;
const CAPSLOCK: u8 = 1 << 3;
const NUMLOCK: u8 = 1 << 4;
const SCROLLLOCK: u8 = 1 << 5;
const E0ESC: u8 = 1 << 6;

// Special keycodes
const KEY_HOME: u8 = 0xE0;
const KEY_END: u8 = 0xE1;
const KEY_UP: u8 = 0xE2;
const KEY_DN: u8 = 0xE3;
const KEY_LF: u8 = 0xE4;
const KEY_RT: u8 = 0xE5;
const KEY_PGUP: u8 = 0xE6;
const KEY_PGDN: u8 = 0xE7;
const KEY_INS: u8 = 0xE8;
const KEY_DEL: u8 = 0xE9;

// c(u8('A')) == Control-A
const fn c(x: u8) -> u8 {
    x - u8('@')
}

const fn u8(x: char) -> u8 {
    x as u8
}

const fn cp<const LEN: usize>(mut code: [u8; 256], init: [u8; LEN]) -> [u8; 256] {
    let mut i = 0;
    while i < LEN {
        code[i] = init[i];
        i += 1;
    }
    code
}

const SHIFT_CODE: [u8; 256] = {
    const fn gen() -> [u8; 256] {
        let mut code = [NO; 256];
        code[0x1D] = CTL;
        code[0x2A] = SHIFT;
        code[0x36] = SHIFT;
        code[0x38] = ALT;
        code[0x9D] = CTL;
        code[0xB8] = ALT;
        code
    }
    gen()
};

const TOGGLE_CODE: [u8; 256] = {
    const fn gen() -> [u8; 256] {
        let mut code = [NO; 256];
        code[0x3A] = CAPSLOCK;
        code[0x45] = NUMLOCK;
        code[0x46] = SCROLLLOCK;
        code
    }
    gen()
};

const NORMALMAP: [u8; 256] = {
    const fn gen() -> [u8; 256] {
        #[rustfmt::skip]
        let init = [
            NO,   0x1B, u8('1'),  u8('2'),  u8('3'),  u8('4'),  u8('5'),  u8('6'),  // 0x00
            u8('7'),  u8('8'),  u8('9'),  u8('0'),  u8('-'),  u8('='),  BS, u8('\t'),
            u8('q'),  u8('w'),  u8('e'),  u8('r'),  u8('t'),  u8('y'),  u8('u'),  u8('i'),  // 0x10
            u8('o'),  u8('p'),  u8('['),  u8(']'),  u8('\n'), NO,   u8('a'),  u8('s'),
            u8('d'),  u8('f'),  u8('g'),  u8('h'),  u8('j'),  u8('k'),  u8('l'),  u8(';'),  // 0x20
            u8('\''), u8('`'),  NO,   u8('\\'), u8('z'),  u8('x'),  u8('c'),  u8('v'),
            u8('b'),  u8('n'),  u8('m'),  u8(','),  u8('.'),  u8('/'),  NO,   u8('*'),  // 0x30
            NO,   u8(' '),  NO,   NO,   NO,   NO,   NO,   NO,
            NO,   NO,   NO,   NO,   NO,   NO,   NO,   u8('7'),  // 0x40
            u8('8'),  u8('9'),  u8('-'),  u8('4'),  u8('5'),  u8('6'),  u8('+'),  u8('1'),
            u8('2'),  u8('3'),  u8('0'),  u8('.'),  NO,   NO,   NO,   NO,   // 0x50
        ];

        let mut code = [NO; 256];
        code[0x9C] = u8('\n'); // KP_Enter
        code[0xB5] = u8('/'); // KP_Div
        code[0xC8] = KEY_UP;
        code[0xD0] = KEY_DN;
        code[0xC9] = KEY_PGUP;
        code[0xD1] = KEY_PGDN;
        code[0xCB] = KEY_LF;
        code[0xCD] = KEY_RT;
        code[0x97] = KEY_HOME;
        code[0xCF] = KEY_END;
        code[0xD2] = KEY_INS;
        code[0xD3] = KEY_DEL;

        cp(code, init)
    }
    gen()
};

const SHIFTMAP: [u8; 256] = {
    const fn gen() -> [u8; 256] {
        #[rustfmt::skip]
        let init = [
            NO,   033,  u8('!'),  u8('@'),  u8('#'),  u8('$'),  u8('%'),  u8('^'),  // 0x00
            u8('&'),  u8('*'),  u8('('),  u8(')'),  u8('_'),  u8('+'),  BS, u8('\t'),
            u8('Q'),  u8('W'),  u8('E'),  u8('R'),  u8('T'),  u8('Y'),  u8('U'),  u8('I'),  // 0x10
            u8('O'),  u8('P'),  u8('{'),  u8('}'),  u8('\n'), NO,   u8('A'),  u8('S'),
            u8('D'),  u8('F'),  u8('G'),  u8('H'),  u8('J'),  u8('K'),  u8('L'),  u8(':'),  // 0x20
            u8('"'),  u8('~'),  NO,   u8('|'),  u8('Z'),  u8('X'),  u8('C'),  u8('V'),
            u8('B'),  u8('N'),  u8('M'),  u8('<'),  u8('>'),  u8('?'),  NO,   u8('*'),  // 0x30
            NO,   u8(' '),  NO,   NO,   NO,   NO,   NO,   NO,
            NO,   NO,   NO,   NO,   NO,   NO,   NO,   u8('7'),  // 0x40
            u8('8'),  u8('9'),  u8('-'),  u8('4'),  u8('5'),  u8('6'),  u8('+'),  u8('1'),
            u8('2'),  u8('3'),  u8('0'),  u8('.'),  NO,   NO,   NO,   NO,   // 0x50
        ];

        let mut code = [NO; 256];
        code[0x9C] = u8('\n'); // KP_Enter
        code[0xB5] = u8('/'); // KP_Div
        code[0xC8] = KEY_UP;
        code[0xD0] = KEY_DN;
        code[0xC9] = KEY_PGUP;
        code[0xD1] = KEY_PGDN;
        code[0xCB] = KEY_LF;
        code[0xCD] = KEY_RT;
        code[0x97] = KEY_HOME;
        code[0xCF] = KEY_END;
        code[0xD2] = KEY_INS;
        code[0xD3] = KEY_DEL;

        cp(code, init)
    }
    gen()
};

const CTLMAP: [u8; 256] = {
    const fn gen() -> [u8; 256] {
        #[rustfmt::skip]
        let init = [
            NO,      NO,      NO,      NO,      NO,      NO,      NO,      NO,
            NO,      NO,      NO,      NO,      NO,      NO,      NO,      NO,
            c(u8('Q')),  c(u8('W')),  c(u8('E')),  c(u8('R')),  c(u8('T')),  c(u8('Y')),  c(u8('U')),  c(u8('I')),
            c(u8('O')),  c(u8('P')),  NO,      NO,      u8('\r'),    NO,      c(u8('A')),  c(u8('S')),
            c(u8('D')),  c(u8('F')),  c(u8('G')),  c(u8('H')),  c(u8('J')),  c(u8('K')),  c(u8('L')),  NO,
            NO,      NO,      NO,      c(u8('\\')), c(u8('Z')),  c(u8('X')),  c(u8('C')),  c(u8('V')),
            c(u8('B')),  c(u8('N')),  c(u8('M')),  NO,      NO,      c(u8('/')),  NO,      NO,
        ];

        let mut code = [NO; 256];
        code[0x9C] = u8('\r'); // KP_Enter
        code[0xB5] = c(u8('/')); // KP_Div
        code[0xC8] = KEY_UP;
        code[0xD0] = KEY_DN;
        code[0xC9] = KEY_PGUP;
        code[0xD1] = KEY_PGDN;
        code[0xCB] = KEY_LF;
        code[0xCD] = KEY_RT;
        code[0x97] = KEY_HOME;
        code[0xCF] = KEY_END;
        code[0xD2] = KEY_INS;
        code[0xD3] = KEY_DEL;

        cp(code, init)
    }
    gen()
};

unsafe fn keyboard_getc() -> Option<u8> {
    static mut SHIFT: u8 = 0;

    let charcode = [&NORMALMAP, &SHIFTMAP, &CTLMAP, &CTLMAP];
    let st = inb(KBSTATP);
    if st & KBS_DIB == 0 {
        return None;
    }

    let mut data = inb(KBDATAP);
    if data == 0xE0 {
        SHIFT |= E0ESC;
        return Some(0);
    } else if data & 0x80 != 0 {
        // Key released
        let data = if SHIFT & E0ESC != 0 {
            data
        } else {
            data & 0x7F
        };
        SHIFT &= !(SHIFT_CODE[data as usize] | E0ESC);
        return Some(0);
    } else if SHIFT & E0ESC != 0 {
        data |= 0x80;
        SHIFT &= !E0ESC;
    }

    SHIFT |= SHIFT_CODE[data as usize];
    SHIFT ^= TOGGLE_CODE[data as usize];

    let mut c = charcode[(SHIFT & (CTL | SHIFT)) as usize][data as usize];
    if SHIFT & CAPSLOCK != 0 {
        if 'a' as u8 <= c && c <= 'z' as u8 {
            c = c + 'A' as u8 - 'a' as u8;
        } else if u8('A') <= c && c <= u8('Z') {
            c += 'a' as u8 - 'A' as u8;
        }
    }

    Some(c)
}

pub fn keyboard_interrupt() {
    extern "C" fn handler() -> u32 {
        match unsafe { keyboard_getc() } {
            Some(v) => v as _,
            None => u32::MAX, // -1
        }
    }

    unsafe {
        consoleintr(handler);
    }
}
