//! Downloading a model, so voice is turned on from the app: from Hugging
//! Face, where whisper.cpp publishes them, checked against the size and
//! SHA-256 [`crate::model`] has for it. It is written beside its place as
//! `.part` and moved there once whole and right: a model file is never
//! there half.

use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use sha2::{Digest, Sha256};

use crate::model::Model;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// No byte for this long: the download has stalled.
const READ_TIMEOUT: Duration = Duration::from_secs(60);

/// Why a model was not downloaded.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The server could not be reached, or said no.
    #[error("{0}")]
    Fetch(String),
    /// What came is not the model: another size, or another sum.
    #[error("what was downloaded is not the model")]
    NotTheModel,
    /// Stopped by the caller.
    #[error("stopped")]
    Stopped,
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

/// Downloads `model` into `dir`, telling `progress` the bytes so far, until
/// it is whole or `stop` is set; where it is then. Blocks: call it on a
/// thread of its own.
pub fn download(
    model: &Model,
    dir: &Path,
    stop: &AtomicBool,
    progress: impl Fn(u64),
) -> Result<PathBuf, Error> {
    let dest = model.path_in(dir);
    from_url(model.url, model.bytes, model.sha256, &dest, stop, &progress)?;
    Ok(dest)
}

fn from_url(
    url: &str,
    bytes: u64,
    sha256: &str,
    dest: &Path,
    stop: &AtomicBool,
    progress: &dyn Fn(u64),
) -> Result<(), Error> {
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()?;
    runtime.block_on(fetch(url, bytes, sha256, dest, stop, progress))
}

async fn fetch(
    url: &str,
    bytes: u64,
    sha256: &str,
    dest: &Path,
    stop: &AtomicBool,
    progress: &dyn Fn(u64),
) -> Result<(), Error> {
    let fail = |e: &dyn std::fmt::Display| Error::Fetch(e.to_string());
    // reqwest on rustls with ring, as the sync places use it. Once is
    // enough; a second time it says it is set already.
    let _ = rustls::crypto::ring::default_provider().install_default();
    let client = reqwest::Client::builder()
        .connect_timeout(CONNECT_TIMEOUT)
        .read_timeout(READ_TIMEOUT)
        .user_agent(concat!("authwing/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| fail(&e))?;
    let mut res = client.get(url).send().await.map_err(|e| fail(&e))?;
    if !res.status().is_success() {
        return Err(Error::Fetch(res.status().to_string()));
    }
    if res.content_length().is_some_and(|n| n != bytes) {
        return Err(Error::NotTheModel);
    }
    if let Some(dir) = dest.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let part = dest.with_extension("part");
    let written = async {
        let mut file = std::fs::File::create(&part)?;
        let mut hash = Sha256::new();
        let mut got = 0u64;
        while let Some(chunk) = res.chunk().await.map_err(|e| fail(&e))? {
            if stop.load(Ordering::Relaxed) {
                return Err(Error::Stopped);
            }
            got += chunk.len() as u64;
            if got > bytes {
                return Err(Error::NotTheModel);
            }
            hash.update(&chunk);
            file.write_all(&chunk)?;
            progress(got);
        }
        file.sync_all()?;
        let sum: String = hash.finalize().iter().map(|b| format!("{b:02x}")).collect();
        if got != bytes || sum != sha256 {
            return Err(Error::NotTheModel);
        }
        Ok(())
    }
    .await;
    match written {
        Ok(()) => Ok(std::fs::rename(&part, dest)?),
        Err(e) => {
            let _ = std::fs::remove_file(&part);
            Err(e)
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::Read as _;

    use super::*;

    /// Serves `body` once on a port of this machine, in pieces; its URL.
    fn serve_once(body: &'static [u8]) -> String {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            let (mut conn, _) = listener.accept().expect("accept");
            let mut req = [0u8; 1024];
            let _ = conn.read(&mut req);
            let head = format!(
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            );
            let _ = conn.write_all(head.as_bytes());
            for piece in body.chunks(4) {
                let _ = conn.write_all(piece);
                let _ = conn.flush();
                std::thread::sleep(Duration::from_millis(5));
            }
        });
        format!("http://{addr}/ggml-test.bin")
    }

    fn sum(data: &[u8]) -> String {
        Sha256::digest(data)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    fn place(test: &str) -> (PathBuf, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "authwing-voice-download-{test}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let dest = dir.join("ggml-test.bin");
        (dir, dest)
    }

    fn left(dir: &Path) -> Vec<PathBuf> {
        std::fs::read_dir(dir)
            .map(|d| d.flatten().map(|e| e.path()).collect())
            .unwrap_or_default()
    }

    const BODY: &[u8] = b"a model, in a few pieces";

    #[test]
    fn a_model_is_kept_whole_and_right() {
        let (dir, dest) = place("right");
        let seen = std::sync::Mutex::new(Vec::new());
        let stop = AtomicBool::new(false);
        from_url(
            &serve_once(BODY),
            BODY.len() as u64,
            &sum(BODY),
            &dest,
            &stop,
            &|n| seen.lock().expect("seen").push(n),
        )
        .expect("download");
        assert_eq!(std::fs::read(&dest).expect("read"), BODY);
        let seen = seen.into_inner().expect("seen");
        assert!(seen.windows(2).all(|w| w[0] < w[1]), "{seen:?}");
        assert_eq!(seen.last(), Some(&(BODY.len() as u64)));
        assert_eq!(left(&dir), [dest]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_wrong_sum_or_size_leaves_nothing() {
        let (dir, dest) = place("wrong");
        let stop = AtomicBool::new(false);
        let wrong_sum = from_url(
            &serve_once(BODY),
            BODY.len() as u64,
            &sum(b"another"),
            &dest,
            &stop,
            &|_| {},
        );
        assert!(
            matches!(wrong_sum, Err(Error::NotTheModel)),
            "{wrong_sum:?}"
        );
        let wrong_size = from_url(&serve_once(BODY), 3, &sum(BODY), &dest, &stop, &|_| {});
        assert!(
            matches!(wrong_size, Err(Error::NotTheModel)),
            "{wrong_size:?}"
        );
        assert!(left(&dir).is_empty(), "{:?}", left(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn a_stopped_download_leaves_nothing() {
        let (dir, dest) = place("stopped");
        let stop = AtomicBool::new(false);
        let stopped = from_url(
            &serve_once(BODY),
            BODY.len() as u64,
            &sum(BODY),
            &dest,
            &stop,
            &|_| stop.store(true, Ordering::Relaxed),
        );
        assert!(matches!(stopped, Err(Error::Stopped)), "{stopped:?}");
        assert!(left(&dir).is_empty(), "{:?}", left(&dir));
        let _ = std::fs::remove_dir_all(&dir);
    }
}
