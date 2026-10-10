//! Shared fuzz helpers for the `mda-core` integration tests. Orchestrator-owned: import with
//! `mod common;`, never edit from a task. Hand-rolled xorshift, fixed seeds: no `rand`.
#![allow(dead_code)] // reason: each test binary uses a subset
#![allow(clippy::indexing_slicing)] // reason: indices are reduced modulo the slice length

/// xorshift64. `next_u64`, not `next`: `clippy::should_implement_trait`.
pub struct XorShift(pub u64);

impl XorShift {
    pub fn next_u64(&mut self) -> u64 {
        // A zero state would stay zero forever.
        let mut x = if self.0 == 0 {
            0x9E37_79B9_7F4A_7C15
        } else {
            self.0
        };
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    /// A value in `0..n` (`n > 0`).
    fn below(&mut self, n: usize) -> usize {
        (self.next_u64() % n as u64) as usize
    }
}

/// The parser's sharp edges: multibyte, line terminators, fences, flags, headings, YAML indicators,
/// token openers, BOM, ESC, DEL, NUL.
pub const FRAGMENTS: &[&str] = &[
    "a",
    "é",
    "日",
    "\u{2028}",
    "\r",
    "\n",
    "\r\n",
    "```",
    "~~~",
    "%%",
    "%%oa:start%%",
    "%%oa:end%%",
    "## ",
    "---",
    "***",
    ":",
    ": ",
    "- ",
    "  ",
    "\t",
    "#",
    " #",
    "'",
    "\"",
    "\\",
    "|",
    "|-",
    "[]",
    "{",
    "&",
    "*",
    "!",
    "<VK-",
    "</VK-",
    "<VK-each:",
    "<VK-end:",
    ">",
    "VK-a",
    "=",
    "\u{feff}",
    "\u{1b}",
    "\u{7f}",
    "\0",
];

/// `n` strings, each 0..=`max_frags` fragments concatenated.
pub fn fuzz_strings(seed: u64, n: usize, max_frags: usize) -> Vec<String> {
    let mut r = XorShift(seed);
    (0..n)
        .map(|_| {
            let len = r.below(max_frags + 1);
            (0..len)
                .map(|_| FRAGMENTS[r.below(FRAGMENTS.len())])
                .collect()
        })
        .collect()
}

/// `n` byte strings of 0..=`max_len` arbitrary bytes (invalid UTF-8 included).
pub fn fuzz_bytes(seed: u64, n: usize, max_len: usize) -> Vec<Vec<u8>> {
    let mut r = XorShift(seed);
    (0..n)
        .map(|_| {
            let len = r.below(max_len + 1);
            (0..len).map(|_| r.next_u64() as u8).collect()
        })
        .collect()
}
