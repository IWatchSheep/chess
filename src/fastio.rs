//! Faster reading and writing of the game files.
//!
//! Rules Step 6 ("Performance and File Management") and Step 15.5 keep the four
//! working files and the archive as text, copied byte for byte. Nothing here
//! changes those bytes or the six-move search. It only changes how they move:
//!
//! * **Buffering.** Writers go through a [`BufWriter`] and a temporary file that
//!   is renamed into place, so a reader never sees a half-written future file.
//!   Readers that only need lines (history `RECORD` lines, the memory list) use
//!   a [`BufReader`] instead of pulling the whole file in as one `String`.
//! * **Memory-mapped reads.** On Unix a future-search file or an archive is mapped
//!   into the process, so the strategy evidence can be parsed without first
//!   copying the sequence listing into a `String`. Windows, and any machine
//!   where the mapping fails, reads the same bytes through a [`BufReader`].
//!   The binary cache and the evidence parser still run on that path.
//! * **Binary evidence cache.** Lists 1–4 need the parsed strategy evidence, not
//!   the sequence listing. That evidence is stored beside the future-search
//!   file (`white_future.txt.evidence`). It is used only when a hash of the
//!   text bytes still matches. Length and modification time are not enough: a
//!   same-size edit in the same timestamp tick must not be served from the
//!   cache. The text file stays the record. The cache is not one of the four
//!   working files and it is not copied into the archive.

use std::fs::{self, File};
use std::io::{self, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

/// Bytes buffered on the way to disk.
pub const WRITE_BUFFER: usize = 256 * 1024;

const CACHE_MAGIC: &[u8; 8] = b"CHEV2\0\0\0";
const CACHE_HEADER: usize = 48;

/// Length and modification time of a text file. Recorded in the cache, but a
/// match here is not enough to trust the cache. [`fingerprint`] of the bytes is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TextStamp {
    len: u64,
    mtime_sec: u64,
    mtime_nsec: u32,
}

/// Two 64-bit mixes of every byte. A same-size edit changes this even when the
/// file's length and modification time do not.
fn fingerprint(bytes: &[u8]) -> [u8; 16] {
    let mut fnv = 0xcbf29ce484222325u64;
    let mut mix = 0x9E3779B185EBCA87u64 ^ (bytes.len() as u64);
    for &b in bytes {
        fnv ^= b as u64;
        fnv = fnv.wrapping_mul(0x100000001b3);
        mix = mix.rotate_left(5) ^ (b as u64).wrapping_mul(0xC2B2AE3D27D4EB4F);
    }
    fnv ^= bytes.len() as u64;
    mix ^= fnv.rotate_left(17);
    let mut out = [0u8; 16];
    out[..8].copy_from_slice(&fnv.to_le_bytes());
    out[8..].copy_from_slice(&mix.to_le_bytes());
    out
}

fn text_stamp(path: &Path) -> io::Result<TextStamp> {
    let meta = fs::metadata(path)?;
    let modified = meta.modified()?;
    let since = modified
        .duration_since(UNIX_EPOCH)
        .map_err(|e| io::Error::new(io::ErrorKind::Other, e))?;
    Ok(TextStamp {
        len: meta.len(),
        mtime_sec: since.as_secs(),
        mtime_nsec: since.subsec_nanos(),
    })
}

fn temp_sibling(path: &Path) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".tmp");
    path.with_file_name(name)
}

/// Write `bytes` through a large buffer, via a temporary file renamed into place.
pub fn write_buffered(path: &Path, bytes: &[u8]) -> io::Result<()> {
    write_buffered_with(path, |w| w.write_all(bytes))
}

/// Stream bytes to `path` the same way as [`write_buffered`].
///
/// The finished file contains exactly what `write` sent. On failure the
/// temporary file is removed and `path` is left as it was.
pub fn write_buffered_with(
    path: &Path,
    write: impl FnOnce(&mut BufWriter<File>) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent)?;
        }
    }
    let tmp = temp_sibling(path);
    let result = (|| {
        let file = File::create(&tmp)?;
        let mut w = BufWriter::with_capacity(WRITE_BUFFER, file);
        write(&mut w)?;
        w.flush()?;
        Ok(())
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(e);
    }
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        e
    })
}

/// Read a text file through a [`BufReader`].
pub fn read_to_string_buffered(path: &Path) -> io::Result<String> {
    let file = File::open(path)?;
    let mut reader = BufReader::with_capacity(WRITE_BUFFER, file);
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    Ok(text)
}

/// A read-only view of a file.
///
/// On Unix this is a memory map. Windows has no `mmap` in this build: the bytes
/// are read through a [`BufReader`] into memory, and every later step (the
/// evidence parser and the binary cache) uses that slice. A failed map on Unix
/// takes the same buffered path. Either way [`as_slice`](MappedFile::as_slice)
/// is the file contents.
pub struct MappedFile {
    ptr: *mut u8,
    len: usize,
    owned: Option<Vec<u8>>,
}

impl MappedFile {
    /// True when these bytes are a Unix memory map, not a buffered copy.
    pub fn is_memory_map(&self) -> bool {
        self.owned.is_none() && !self.ptr.is_null() && self.len > 0
    }

    pub fn as_slice(&self) -> &[u8] {
        if let Some(owned) = &self.owned {
            return owned;
        }
        if self.ptr.is_null() || self.len == 0 {
            return &[];
        }
        // Safety: `ptr` came from mmap of `len` bytes, is still mapped, and is
        // not mutated. The slice ends when this value is dropped.
        unsafe { std::slice::from_raw_parts(self.ptr, self.len) }
    }
}

impl Drop for MappedFile {
    fn drop(&mut self) {
        #[cfg(unix)]
        if self.owned.is_none() && !self.ptr.is_null() && self.len > 0 {
            unsafe { unmap(self.ptr, self.len) };
        }
    }
}

/// Map `path` for reading.
pub fn map_file(path: &Path) -> io::Result<MappedFile> {
    let file = File::open(path)?;
    let len = usize::try_from(file.metadata()?.len())
        .map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    if len == 0 {
        return Ok(MappedFile {
            ptr: std::ptr::null_mut(),
            len: 0,
            owned: None,
        });
    }
    if let Some(ptr) = map_fd(&file, len) {
        drop(file);
        return Ok(MappedFile {
            ptr,
            len,
            owned: None,
        });
    }
    let mut owned = Vec::new();
    let mut reader = BufReader::with_capacity(WRITE_BUFFER.min(len.max(1)), file);
    reader.read_to_end(&mut owned)?;
    Ok(MappedFile {
        ptr: std::ptr::null_mut(),
        len: owned.len(),
        owned: Some(owned),
    })
}

#[cfg(unix)]
fn map_fd(file: &File, len: usize) -> Option<*mut u8> {
    use std::os::unix::io::AsRawFd;
    // PROT_READ and MAP_PRIVATE have these values on Linux and the BSDs, including macOS.
    const PROT_READ: i32 = 1;
    const MAP_PRIVATE: i32 = 2;
    extern "C" {
        fn mmap(addr: *mut u8, len: usize, prot: i32, flags: i32, fd: i32, offset: i64) -> *mut u8;
    }
    // Safety: the file is open for reading and `len` is its length. A failed
    // map returns the all-ones pointer, which we treat as "use the buffered read".
    let ptr = unsafe {
        mmap(
            std::ptr::null_mut(),
            len,
            PROT_READ,
            MAP_PRIVATE,
            file.as_raw_fd(),
            0,
        )
    };
    if ptr.is_null() || ptr as isize == -1 {
        None
    } else {
        Some(ptr)
    }
}

#[cfg(not(unix))]
fn map_fd(_file: &File, _len: usize) -> Option<*mut u8> {
    None
}

#[cfg(unix)]
unsafe fn unmap(ptr: *mut u8, len: usize) {
    extern "C" {
        fn munmap(addr: *mut u8, len: usize) -> i32;
    }
    // Safety: `ptr`/`len` are the pair returned by a successful mmap in this process.
    unsafe { munmap(ptr, len) };
}

/// Sidecar path for the binary evidence of a future-search file.
pub fn evidence_cache_path(text_path: &Path) -> PathBuf {
    let mut name = text_path.file_name().unwrap_or_default().to_os_string();
    name.push(".evidence");
    text_path.with_file_name(name)
}

/// Delete a sidecar. Missing files are ignored. Called before a future-search
/// file is replaced, so a new placeholder is never paired with an old cache.
pub fn remove_evidence_cache(text_path: &Path) {
    let _ = fs::remove_file(evidence_cache_path(text_path));
}

/// The binary payload saved for `text_path`, if it is still a parse of `text_bytes`.
///
/// `text_bytes` must be the current contents of the text file. Length and
/// modification time are not consulted for the accept/reject decision: the
/// stored fingerprint has to equal [`fingerprint`] of these bytes. `Ok(None)`
/// means "parse the text".
pub fn read_evidence_cache(text_path: &Path, text_bytes: &[u8]) -> io::Result<Option<Vec<u8>>> {
    let bytes = match fs::read(evidence_cache_path(text_path)) {
        Ok(bytes) => bytes,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if bytes.len() < CACHE_HEADER || bytes[..8] != CACHE_MAGIC[..] {
        return Ok(None);
    }
    let len = u64::from_le_bytes(bytes[8..16].try_into().unwrap());
    if len != text_bytes.len() as u64 || bytes[32..48] != fingerprint(text_bytes) {
        return Ok(None);
    }
    Ok(Some(bytes[CACHE_HEADER..].to_vec()))
}

/// Write the binary payload for the text file as it stands right now.
///
/// The file is read back so the fingerprint is of the bytes on disk, not of a
/// buffer that might already have been replaced.
pub fn write_evidence_cache(text_path: &Path, payload: &[u8]) -> io::Result<()> {
    let mapped = map_file(text_path)?;
    let text_bytes = mapped.as_slice();
    let stamp = text_stamp(text_path)?;
    if stamp.len != text_bytes.len() as u64 {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "future file changed while its evidence cache was written",
        ));
    }
    let hash = fingerprint(text_bytes);
    let mut bytes = Vec::with_capacity(CACHE_HEADER + payload.len());
    bytes.extend_from_slice(CACHE_MAGIC);
    bytes.extend_from_slice(&stamp.len.to_le_bytes());
    bytes.extend_from_slice(&stamp.mtime_sec.to_le_bytes());
    bytes.extend_from_slice(&stamp.mtime_nsec.to_le_bytes());
    bytes.extend_from_slice(&0u32.to_le_bytes());
    bytes.extend_from_slice(&hash);
    bytes.extend_from_slice(payload);
    write_buffered(&evidence_cache_path(text_path), &bytes)
}
