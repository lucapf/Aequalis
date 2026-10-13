//! Gzip decompression (RFC 1952 around RFC 1951 DEFLATE), streamed: from a reader to a writer
//! through the 32 KiB window DEFLATE needs, so a large archive (the city archive: ~47 MB out)
//! takes little memory. No `gzip` program: the iPhone app can't start one. Huffman codes up to
//! `FAST` bits are looked up in a table; longer ones are decoded bit by bit (as zlib's `puff`).

use std::io::{BufRead, BufReader, Read, Write};

const WINDOW: usize = 32 * 1024;
/// Written out in chunks of this size.
const CHUNK: usize = 64 * 1024;
/// Codes this long or shorter are found with one table lookup.
const FAST: u32 = 9;

/// Decompresses the gzip stream `input` into `output`; returns how many bytes were written. The
/// size and CRC-32 in the trailer are checked.
pub fn gunzip(input: impl Read, output: impl Write) -> Result<u64, String> {
    let mut d = Decoder { bits: Bits { input: BufReader::new(input), buf: 0, count: 0 }, out: Out::new(output) };
    d.header()?;
    loop {
        let last = d.bits.need(1)?;
        match d.bits.need(2)? {
            0 => d.stored()?,
            1 => d.fixed()?,
            2 => d.dynamic()?,
            _ => return Err("gzip: invalid block type".into()),
        }
        if last == 1 {
            break;
        }
    }
    d.out.flush()?;
    // The trailer, byte aligned: CRC-32 and size (mod 2³²), little-endian.
    d.bits.align();
    let crc = d.bits.u32()?;
    let size = d.bits.u32()?;
    if crc != d.out.crc ^ 0xFFFF_FFFF {
        return Err("gzip: CRC mismatch (damaged download)".into());
    }
    if size != d.out.total as u32 {
        return Err("gzip: size mismatch (damaged download)".into());
    }
    Ok(d.out.total)
}

/// Bits from the input, least significant first.
struct Bits<R: Read> {
    input: BufReader<R>,
    buf: u32,
    count: u32,
}

impl<R: Read> Bits<R> {
    /// The next whole byte (only while aligned): buffered bits first (`fill` reads ahead).
    fn byte(&mut self) -> Result<u8, String> {
        if self.count >= 8 {
            let b = self.buf as u8;
            self.buf >>= 8;
            self.count -= 8;
            return Ok(b);
        }
        self.raw()
    }

    /// The next byte of the input.
    fn raw(&mut self) -> Result<u8, String> {
        let available = self.input.fill_buf().map_err(|e| format!("gzip: {e}"))?;
        let Some(&b) = available.first() else { return Err("gzip: the data ends too early".into()) };
        self.input.consume(1);
        Ok(b)
    }

    /// The next `n` bits (at most 16) as a number.
    fn need(&mut self, n: u32) -> Result<u32, String> {
        while self.count < n {
            self.buf |= u32::from(self.raw()?) << self.count;
            self.count += 8;
        }
        let v = self.buf & ((1u32 << n) - 1);
        self.buf >>= n;
        self.count -= n;
        Ok(v)
    }

    /// At least `n` bits buffered, unless the input ends first.
    fn fill(&mut self, n: u32) -> bool {
        while self.count < n {
            match self.input.fill_buf() {
                Ok([b, ..]) => {
                    self.buf |= u32::from(*b) << self.count;
                    self.count += 8;
                    self.input.consume(1);
                }
                _ => return false,
            }
        }
        true
    }

    /// Drops the bits left of the current byte (whole bytes read ahead stay).
    fn align(&mut self) {
        let partial = self.count % 8;
        self.buf >>= partial;
        self.count -= partial;
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from(self.byte()?) | u16::from(self.byte()?) << 8)
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from(self.u16()?) | u32::from(self.u16()?) << 16)
    }
}

/// The output: the window (for back references), the CRC-32 and size so far, and a buffer
/// written out in chunks.
struct Out<W: Write> {
    output: W,
    window: Vec<u8>,
    total: u64,
    crc: u32,
    pending: Vec<u8>,
}

impl<W: Write> Out<W> {
    fn new(output: W) -> Self {
        Out { output, window: vec![0; WINDOW], total: 0, crc: 0xFFFF_FFFF, pending: Vec::with_capacity(CHUNK) }
    }

    fn push(&mut self, b: u8) -> Result<(), String> {
        self.window[self.total as usize % WINDOW] = b;
        self.total += 1;
        self.pending.push(b);
        if self.pending.len() >= CHUNK {
            self.flush()?;
        }
        Ok(())
    }

    /// `len` bytes from `distance` back.
    fn copy(&mut self, distance: usize, len: usize) -> Result<(), String> {
        if distance == 0 || distance > WINDOW || distance as u64 > self.total {
            return Err("gzip: invalid distance".into());
        }
        for _ in 0..len {
            let b = self.window[(self.total as usize - distance) % WINDOW];
            self.push(b)?;
        }
        Ok(())
    }

    fn flush(&mut self) -> Result<(), String> {
        let table = crc_table();
        for &b in &self.pending {
            self.crc = table[((self.crc ^ u32::from(b)) & 0xFF) as usize] ^ (self.crc >> 8);
        }
        self.output.write_all(&self.pending).map_err(|e| format!("gzip: {e}"))?;
        self.pending.clear();
        Ok(())
    }
}

fn crc_table() -> &'static [u32; 256] {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (n, entry) in t.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 == 1 { 0xEDB8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *entry = c;
        }
        t
    })
}

/// A canonical Huffman code: how many codes of each length, the symbols in code order, and the
/// lookup table for the short codes: (symbol, length) by the next `FAST` input bits (length 0:
/// longer than `FAST`).
struct Huffman {
    count: [u16; 16],
    symbol: Vec<u16>,
    fast: Vec<(u16, u8)>,
}

impl Huffman {
    /// The code for these code lengths (0: unused symbol), and how many codes are left unused
    /// (negative: over-subscribed, an invalid code).
    fn new(lengths: &[u8]) -> (Huffman, i32) {
        let mut count = [0u16; 16];
        for &l in lengths {
            count[l as usize] += 1;
        }
        let mut left = 1i32;
        for &c in &count[1..] {
            left = (left << 1) - i32::from(c);
            if left < 0 {
                return (Huffman { count, symbol: Vec::new(), fast: Vec::new() }, left);
            }
        }
        let mut offsets = [0u16; 16];
        for len in 1..15 {
            offsets[len + 1] = offsets[len] + count[len];
        }
        let mut symbol = vec![0u16; lengths.len()];
        for (s, &l) in lengths.iter().enumerate() {
            if l != 0 {
                symbol[offsets[l as usize] as usize] = s as u16;
                offsets[l as usize] += 1;
            }
        }
        // The canonical codes (first code of each length), then every short code's entries: its
        // bits reversed (the input comes least significant bit first), whatever follows them.
        let mut next = [0u32; 16];
        let mut code = 0u32;
        for len in 1..16 {
            code = (code + u32::from(if len > 1 { count[len - 1] } else { 0 })) << 1;
            next[len] = code;
        }
        let mut fast = vec![(0u16, 0u8); 1 << FAST];
        for (s, &l) in lengths.iter().enumerate() {
            let l = u32::from(l);
            if l == 0 {
                continue;
            }
            let c = next[l as usize];
            next[l as usize] += 1;
            if l <= FAST {
                let reversed = (c.reverse_bits() >> (32 - l)) as usize;
                for i in (reversed..1 << FAST).step_by(1 << l) {
                    fast[i] = (s as u16, l as u8);
                }
            }
        }
        (Huffman { count, symbol, fast }, left)
    }
}

const LEN_BASE: [u16; 29] = [3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131, 163, 195, 227, 258];
const LEN_EXTRA: [u8; 29] = [0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0];
const DIST_BASE: [u16; 30] =
    [1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537, 2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577];
const DIST_EXTRA: [u8; 30] = [0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13, 13];
/// The order of the code length code lengths in a dynamic block.
const ORDER: [usize; 19] = [16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15];

struct Decoder<R: Read, W: Write> {
    bits: Bits<R>,
    out: Out<W>,
}

impl<R: Read, W: Write> Decoder<R, W> {
    fn header(&mut self) -> Result<(), String> {
        if self.bits.byte()? != 0x1f || self.bits.byte()? != 0x8b {
            return Err("not gzip data".into());
        }
        if self.bits.byte()? != 8 {
            return Err("gzip: unknown compression method".into());
        }
        let flags = self.bits.byte()?;
        // Time, extra flags, system.
        for _ in 0..6 {
            self.bits.byte()?;
        }
        if flags & 4 != 0 {
            for _ in 0..self.bits.u16()? {
                self.bits.byte()?;
            }
        }
        // A file name, a comment: zero-terminated.
        for flag in [8, 16] {
            if flags & flag != 0 {
                while self.bits.byte()? != 0 {}
            }
        }
        if flags & 2 != 0 {
            self.bits.u16()?;
        }
        Ok(())
    }

    fn decode(&mut self, h: &Huffman) -> Result<u16, String> {
        if self.bits.fill(FAST) {
            let (symbol, len) = h.fast[(self.bits.buf & ((1 << FAST) - 1)) as usize];
            if len > 0 {
                self.bits.buf >>= len;
                self.bits.count -= u32::from(len);
                return Ok(symbol);
            }
        }
        let (mut code, mut first, mut index) = (0i32, 0i32, 0i32);
        for len in 1..16 {
            code |= self.bits.need(1)? as i32;
            let count = i32::from(h.count[len]);
            if code - count < first {
                return Ok(h.symbol[(index + code - first) as usize]);
            }
            index += count;
            first = (first + count) << 1;
            code <<= 1;
        }
        Err("gzip: invalid code".into())
    }

    fn stored(&mut self) -> Result<(), String> {
        self.bits.align();
        let len = self.bits.u16()?;
        if self.bits.u16()? != !len {
            return Err("gzip: invalid stored block".into());
        }
        for _ in 0..len {
            let b = self.bits.byte()?;
            self.out.push(b)?;
        }
        Ok(())
    }

    fn codes(&mut self, lengths: &Huffman, distances: &Huffman) -> Result<(), String> {
        loop {
            let symbol = self.decode(lengths)?;
            match symbol {
                0..=255 => self.out.push(symbol as u8)?,
                256 => return Ok(()),
                _ => {
                    let s = usize::from(symbol - 257);
                    if s >= LEN_BASE.len() {
                        return Err("gzip: invalid length".into());
                    }
                    let len = usize::from(LEN_BASE[s]) + self.bits.need(u32::from(LEN_EXTRA[s]))? as usize;
                    let d = usize::from(self.decode(distances)?);
                    if d >= DIST_BASE.len() {
                        return Err("gzip: invalid distance code".into());
                    }
                    let distance = usize::from(DIST_BASE[d]) + self.bits.need(u32::from(DIST_EXTRA[d]))? as usize;
                    self.out.copy(distance, len)?;
                }
            }
        }
    }

    fn fixed(&mut self) -> Result<(), String> {
        let mut lengths = [0u8; 288];
        lengths[..144].fill(8);
        lengths[144..256].fill(9);
        lengths[256..280].fill(7);
        lengths[280..].fill(8);
        let (lens, _) = Huffman::new(&lengths);
        let (dists, _) = Huffman::new(&[5u8; 30]);
        self.codes(&lens, &dists)
    }

    fn dynamic(&mut self) -> Result<(), String> {
        let nlen = self.bits.need(5)? as usize + 257;
        let ndist = self.bits.need(5)? as usize + 1;
        let ncode = self.bits.need(4)? as usize + 4;
        if nlen > 286 || ndist > 30 {
            return Err("gzip: invalid block header".into());
        }
        let mut lengths = [0u8; 320];
        for &i in &ORDER[..ncode] {
            lengths[i] = self.bits.need(3)? as u8;
        }
        let (code_lengths, left) = Huffman::new(&lengths[..19]);
        if left != 0 {
            return Err("gzip: invalid code lengths".into());
        }
        let mut i = 0;
        while i < nlen + ndist {
            let symbol = self.decode(&code_lengths)?;
            if symbol < 16 {
                lengths[i] = symbol as u8;
                i += 1;
                continue;
            }
            let (value, repeat) = match symbol {
                16 if i == 0 => return Err("gzip: repeat with no length".into()),
                16 => (lengths[i - 1], 3 + self.bits.need(2)?),
                17 => (0, 3 + self.bits.need(3)?),
                _ => (0, 11 + self.bits.need(7)?),
            };
            let repeat = repeat as usize;
            if i + repeat > nlen + ndist {
                return Err("gzip: too many lengths".into());
            }
            lengths[i..i + repeat].fill(value);
            i += repeat;
        }
        if lengths[256] == 0 {
            return Err("gzip: no end-of-block code".into());
        }
        // An incomplete code is only allowed when it has a single code (RFC 1951).
        let (lens, left) = Huffman::new(&lengths[..nlen]);
        if left < 0 || (left > 0 && nlen as u16 != lens.count[0] + lens.count[1]) {
            return Err("gzip: invalid literal/length code".into());
        }
        let (dists, left) = Huffman::new(&lengths[nlen..nlen + ndist]);
        if left < 0 || (left > 0 && ndist as u16 != dists.count[0] + dists.count[1]) {
            return Err("gzip: invalid distance code".into());
        }
        self.codes(&lens, &dists)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::process::{Command, Stdio};

    /// `data` through the system's `gzip` (at `level`), then back through `gunzip`.
    fn round_trip(data: &[u8], level: &str) {
        let mut child = Command::new("gzip").args(["-c", level]).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("gzip for the tests");
        let mut stdin = child.stdin.take().unwrap();
        let input = data.to_vec();
        let writer = std::thread::spawn(move || stdin.write_all(&input));
        let compressed = child.wait_with_output().unwrap().stdout;
        writer.join().unwrap().unwrap();
        let mut out = Vec::new();
        assert_eq!(gunzip(&compressed[..], &mut out).unwrap(), data.len() as u64, "{level}");
        assert!(out == data, "{level}: {} bytes differ", data.len());
    }

    /// Pseudo-random bytes (incompressible: stored blocks).
    fn noise(n: usize) -> Vec<u8> {
        let mut x: u64 = 0x2545_F491_4F6C_DD1D;
        (0..n)
            .map(|_| {
                x ^= x << 13;
                x ^= x >> 7;
                x ^= x << 17;
                x as u8
            })
            .collect()
    }

    #[test]
    fn every_kind_of_block() {
        round_trip(b"", "-6");
        round_trip(b"a", "-6");
        // Short text: fixed codes; repeated text: long back references.
        round_trip(b"hello, hello, hello world", "-9");
        let text: Vec<u8> = (0..20_000).flat_map(|i| format!("{i}\tRome\tItaly\t41.89\t12.48\n").into_bytes()).collect();
        for level in ["-1", "-6", "-9"] {
            round_trip(&text, level);
        }
        // Incompressible: stored blocks; and mixed, across the 32 KiB window.
        round_trip(&noise(200_000), "-6");
        let mixed: Vec<u8> = noise(50_000).into_iter().chain(text.iter().copied().take(100_000)).chain(noise(40_000)).collect();
        round_trip(&mixed, "-9");
        // The sample city archive.
        round_trip(crate::cities::tests::SAMPLE.as_bytes(), "-9");
    }

    /// The real city archive (`TALLY_ARCHIVE_GZ=<cities.json.gz>`), timed: `cargo test --release
    /// real_archive_speed -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn real_archive_speed() {
        let path = std::env::var("TALLY_ARCHIVE_GZ").expect("TALLY_ARCHIVE_GZ");
        let started = std::time::Instant::now();
        let n = gunzip(std::fs::File::open(path).unwrap(), std::io::sink()).unwrap();
        eprintln!("{n} bytes in {:.2} s", started.elapsed().as_secs_f64());
    }

    #[test]
    fn damaged_data_is_an_error() {
        let mut child = Command::new("gzip").arg("-c").stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        child.stdin.take().unwrap().write_all(b"the city archive, the city archive").unwrap();
        let good = child.wait_with_output().unwrap().stdout;
        let mut out = Vec::new();
        assert!(gunzip(&b"plain text"[..], &mut out).unwrap_err().contains("not gzip"));
        assert!(gunzip(&good[..good.len() - 4], &mut out).is_err(), "cut short");
        let mut bad_crc = good.clone();
        let at = bad_crc.len() - 8;
        bad_crc[at] ^= 0xFF;
        assert!(gunzip(&bad_crc[..], &mut Vec::new()).unwrap_err().contains("CRC"));
    }
}
