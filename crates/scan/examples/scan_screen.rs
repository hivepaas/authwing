//! Prints the QR codes on the screen, to try `screen` on a system by hand:
//! show a QR code, then `cargo run -p authwing-scan --example scan_screen`.
//!
//! On macOS, without the Screen Recording permission, it says so and stops;
//! `--ask` has macOS ask for it (for the terminal, which the example runs
//! in), to take effect when the terminal next starts.

use authwing_scan::Access;

fn main() {
    let access = authwing_scan::access();
    println!("access: {access:?}");
    if access == Access::NotGranted {
        if std::env::args().any(|arg| arg == "--ask") {
            println!("asked: {:?}", authwing_scan::ask_access());
        } else {
            println!("not allowed to capture the screen: run with --ask to ask");
        }
        return;
    }
    let started = std::time::Instant::now();
    match authwing_scan::screen() {
        Ok(found) => {
            println!("{} QR code(s) in {:?}", found.len(), started.elapsed());
            for found in &found {
                println!("display {}: {}", found.display, found.text);
            }
            let texts: Vec<String> = found.into_iter().map(|found| found.text).collect();
            println!("two-factor: {:?}", authwing_scan::two_factor(&texts));
        }
        Err(error) => println!("error: {error}"),
    }
}
