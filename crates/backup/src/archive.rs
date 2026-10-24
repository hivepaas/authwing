//! ZIP archives, read in memory: 1Password's, Proton Pass's, Dashlane's and
//! Raivo OTP's exports come in one. Which export an archive is shows in the
//! names of its entries, which are in the clear even when what they hold
//! is encrypted (Raivo's, with WinZip's AES).

use std::io::{Cursor, Read};

use zeroize::Zeroizing;
use zip::ZipArchive;
use zip::result::ZipError;

use crate::{Error, Result, damaged};

/// The most an entry may hold, unpacked: 64 MiB, more than the codes and
/// logins of any export take. An archive damaged, or made to, should not
/// fill the memory.
const MAX_ENTRY: u64 = 64 << 20;

/// Whether `bytes` start as a ZIP archive does, with a file's header.
pub(crate) fn is_zip(bytes: &[u8]) -> bool {
    bytes.starts_with(b"PK\x03\x04")
}

/// A ZIP archive in memory.
pub(crate) struct Archive<'a> {
    zip: ZipArchive<Cursor<&'a [u8]>>,
}

impl<'a> Archive<'a> {
    pub(crate) fn open(bytes: &'a [u8]) -> Result<Archive<'a>> {
        let zip = ZipArchive::new(Cursor::new(bytes)).map_err(unzip_error)?;
        Ok(Archive { zip })
    }

    /// Whether the archive holds an entry by the path `name`.
    pub(crate) fn has(&self, name: &str) -> bool {
        self.zip.index_for_name(name).is_some()
    }

    /// Whether the entry `name` is encrypted, and needs a password to read.
    pub(crate) fn is_encrypted(&self, name: &str) -> bool {
        self.zip
            .index_for_name(name)
            .and_then(|index| self.zip.by_index_data(index).ok())
            .is_some_and(|entry| entry.encrypted())
    }

    /// What the entry `name` holds, decrypted with `password` when it is
    /// encrypted. A password that does not open it is the wrong one; one
    /// that seems to but does not give what was sealed means damage.
    pub(crate) fn read(
        &mut self,
        name: &str,
        password: Option<&str>,
    ) -> Result<Zeroizing<Vec<u8>>> {
        let index = self
            .zip
            .index_for_name(name)
            .ok_or_else(|| damaged(format!("{name} is missing")))?;
        let size = self.zip.by_index_data(index).map_err(unzip_error)?.size();
        if size > MAX_ENTRY {
            return Err(damaged(format!("{name} is too large")));
        }
        let entry = match password {
            Some(password) => self.zip.by_index_decrypt(index, password.as_bytes()),
            None => self.zip.by_index(index),
        };
        let mut content = Zeroizing::new(Vec::with_capacity(size as usize));
        // The entry stops at the size it gives; the limit is for one that
        // gives none.
        entry
            .map_err(unzip_error)?
            .take(MAX_ENTRY)
            .read_to_end(&mut content)
            .map_err(damaged)?;
        Ok(content)
    }
}

/// What a ZIP's failure means for the file.
fn unzip_error(e: ZipError) -> Error {
    match e {
        ZipError::InvalidPassword => Error::WrongPassword,
        ZipError::UnsupportedArchive(why) => {
            Error::Unsupported(format!("this archive cannot be read here ({why})"))
        }
        ZipError::CompressionMethodNotSupported(method) => Error::Unsupported(format!(
            "this archive is compressed in a way authwing does not read (method {method})"
        )),
        e => damaged(e),
    }
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    use super::*;
    use crate::testing::zipped;

    #[test]
    fn reads_entries_with_and_without_a_password() {
        let plain = zipped(&[("a.txt", b"plain"), ("dir/b.txt", b"nested")], None);
        assert!(is_zip(&plain));
        let mut archive = Archive::open(&plain).expect("open");
        assert!(archive.has("dir/b.txt") && !archive.has("b.txt"));
        assert!(!archive.is_encrypted("a.txt"));
        let read = |archive: &mut Archive, name, password| {
            archive.read(name, password).map(|content| content.to_vec())
        };
        assert_eq!(read(&mut archive, "a.txt", None).expect("read"), b"plain");
        // A password an entry does not need is not used.
        assert_eq!(
            read(&mut archive, "dir/b.txt", Some("x")).expect("read"),
            b"nested"
        );
        assert!(matches!(
            read(&mut archive, "c.txt", None),
            Err(Error::Damaged(_))
        ));

        let sealed = zipped(&[("codes.json", b"[]")], Some("test"));
        let mut archive = Archive::open(&sealed).expect("open");
        assert!(archive.is_encrypted("codes.json"));
        assert!(matches!(
            read(&mut archive, "codes.json", None),
            Err(Error::Unsupported(_))
        ));
        assert!(matches!(
            read(&mut archive, "codes.json", Some("tset")),
            Err(Error::WrongPassword)
        ));
        assert_eq!(
            read(&mut archive, "codes.json", Some("test")).expect("read"),
            b"[]"
        );
    }

    #[test]
    fn damage_is_told() {
        let mut zip = ZipWriter::new(Cursor::new(Vec::new()));
        let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
        zip.start_file("a.txt", stored).expect("entry");
        zip.write_all(b"the codes, as they were").expect("write");
        let mut bytes = zip.finish().expect("zip").into_inner();
        assert!(matches!(
            Archive::open(&bytes[..bytes.len() - 10]),
            Err(Error::Damaged(_))
        ));
        let at = bytes
            .windows(9)
            .position(|window| window == b"the codes")
            .expect("stored");
        bytes[at] ^= 1;
        let mut archive = Archive::open(&bytes).expect("open");
        assert!(matches!(
            archive.read("a.txt", None),
            Err(Error::Damaged(_))
        ));
    }
}
