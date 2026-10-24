//! Prints the active window, to try `window::active` on a system by hand:
//! `cargo run --example active`, then switch apps within the second.

fn main() {
    std::thread::sleep(std::time::Duration::from_secs(1));
    println!(
        "title access: {:?}",
        authwing_context::window::title_access()
    );
    println!("active: {:?}", authwing_context::window::active());
}
