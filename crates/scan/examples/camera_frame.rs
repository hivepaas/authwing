//! Opens a camera for a few frames, says what it read, and keeps the last
//! preview: for trying the camera out by hand.
//!
//! cargo run -p authwing-scan --features camera --example camera_frame -- [index] [preview.png]

use std::time::{Duration, Instant};

use authwing_scan::camera::{self, Seen};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let index: usize = args.next().map(|a| a.parse()).transpose()?.unwrap_or(0);
    let out = args.next().unwrap_or_else(|| "preview.png".into());
    println!("cameras: {:?}", camera::cameras());
    let (tx, rx) = std::sync::mpsc::channel();
    let started = Instant::now();
    let cam = camera::open(index, move |seen| {
        let _ = tx.send(seen);
    })?;
    let mut frames = 0;
    let mut last = None;
    while let Ok(seen) = rx.recv_timeout(Duration::from_secs(15)) {
        match seen {
            Seen::Frame { preview, texts } => {
                frames += 1;
                if frames == 1 {
                    println!("first frame after {:?}", started.elapsed());
                }
                if !texts.is_empty() {
                    println!("read: {texts:?}");
                }
                last = Some(preview);
                if frames >= 8 {
                    break;
                }
            }
            Seen::Failed(e) => {
                println!("failed: {e}");
                break;
            }
        }
    }
    drop(cam);
    println!("{frames} frames in {:?}", started.elapsed());
    if let Some(p) = last {
        println!("preview {}x{} -> {out}", p.width(), p.height());
        p.save(&out)?;
    }
    Ok(())
}
