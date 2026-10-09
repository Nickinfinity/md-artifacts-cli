//! Bounded NDJSON line reader: the cap is enforced while reading, so a hostile client cannot make
//! the engine buffer more than [`MAX_LINE_BYTES`] + 1 bytes.

use std::io::{self, BufRead, Read};

/// Longest accepted line, counting content bytes only (not the `\n`).
// ponytail: a 16 MiB frame can still expand ~16x into serde_json::Value; lower MAX_LINE_BYTES or stream-parse if memory matters
pub const MAX_LINE_BYTES: usize = 16 << 20;

/// One framed input line.
#[derive(Debug, PartialEq, Eq)]
pub enum Line {
    /// End of input.
    Eof,
    /// A line, `\n` and a trailing `\r` stripped. May be empty.
    Text(Vec<u8>),
    /// The line exceeded [`MAX_LINE_BYTES`]; it was discarded up to its newline.
    TooLong,
}

/// Read the next line from `input`.
pub fn read_line(input: &mut impl BufRead) -> io::Result<Line> {
    let mut buf = Vec::new();
    // +1 so a full cap of content plus its `\n` fits, and one more content byte is detectable.
    let n = input
        .by_ref()
        .take(MAX_LINE_BYTES as u64 + 1)
        .read_until(b'\n', &mut buf)?;
    if n == 0 {
        return Ok(Line::Eof);
    }
    if buf.last() == Some(&b'\n') {
        buf.pop();
    } else if n > MAX_LINE_BYTES {
        discard_to_newline(input)?;
        return Ok(Line::TooLong);
    }
    if buf.last() == Some(&b'\r') {
        buf.pop();
    }
    Ok(Line::Text(buf))
}

/// Drop input through the next `\n` (or EOF) in bounded chunks.
fn discard_to_newline(input: &mut impl BufRead) -> io::Result<()> {
    let mut chunk = Vec::new();
    loop {
        chunk.clear();
        let n = input.by_ref().take(8192).read_until(b'\n', &mut chunk)?;
        if n == 0 || chunk.last() == Some(&b'\n') {
            return Ok(());
        }
    }
}
