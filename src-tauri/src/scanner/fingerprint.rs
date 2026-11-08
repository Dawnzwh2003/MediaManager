use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read, Seek, SeekFrom},
    path::Path,
    sync::atomic::{AtomicBool, Ordering},
};
const MIB: u64 = 1024 * 1024;
/// Versioned, non-overlapping samples. At most 3 MiB, including for sparse videos.
pub fn quick(path: &Path, size: u64, cancel: &AtomicBool) -> io::Result<String> {
    let mut file = File::open(path)?;
    let ranges = if size <= 3 * MIB {
        vec![(0, size)]
    } else {
        vec![(0, MIB), ((size - MIB) / 2, MIB), (size - MIB, MIB)]
    };
    let mut hash = Sha256::new();
    hash.update(b"media-quick-v1");
    hash.update(size.to_le_bytes());
    let mut offsets = String::new();
    let mut buffer = [0u8; 65536];
    for (offset, len) in ranges {
        offsets.push_str(&format!("{offset}+{len},"));
        hash.update(offset.to_le_bytes());
        hash.update(len.to_le_bytes());
        file.seek(SeekFrom::Start(offset))?;
        let mut remaining = len;
        while remaining > 0 {
            if cancel.load(Ordering::Relaxed) {
                return Err(io::Error::new(io::ErrorKind::Interrupted, "cancelled"));
            }
            let count = remaining.min(buffer.len() as u64) as usize;
            file.read_exact(&mut buffer[..count])?;
            hash.update(&buffer[..count]);
            remaining -= count as u64;
        }
    }
    Ok(format!("quick-v1:{size}:{offsets}:{:x}", hash.finalize()))
}
