//! A stand-in for authwing, to try the extension without the app: started
//! by a browser it is the host, as authwing's `main` would be; started by
//! hand it listens as the app does, prints what the extension says, and
//! fills `--fill <code>` into a tab that has a code field.
//!
//! `AUTHWING_BRIDGE_SOCKET` puts the socket elsewhere than the user's, for
//! a browser with a profile of its own (this example only: authwing has
//! no such setting). `stand_in manifest` prints Chrome's host manifest for
//! this program.

use std::ffi::OsString;
use std::process::ExitCode;

use authwing_bridge::endpoint::Endpoint;
use authwing_bridge::host::{self, is_native_host_invocation, run_native_host};
use authwing_bridge::install::{Browser, Manifest};
use authwing_bridge::listener::{Event, Listener};
use authwing_bridge::protocol::{Caller, FromExtension, ToExtension};

fn main() -> ExitCode {
    let args: Vec<OsString> = std::env::args_os().skip(1).collect();
    // First, before anything is written to stdout, which is the browser's.
    if is_native_host_invocation(&args) {
        return match endpoint() {
            Some(endpoint) => {
                let caller = host::caller(&args).unwrap_or(Caller::Unknown);
                let done = host::relay(
                    caller,
                    &endpoint,
                    std::io::stdin(),
                    std::io::stdout().lock(),
                );
                ExitCode::from(u8::from(done.is_err()))
            }
            None => ExitCode::from(run_native_host(&args) as u8),
        };
    }
    if args.first().is_some_and(|a| a == "manifest") {
        let program = std::env::current_exe().expect("this program");
        print!("{}", Manifest::new(Browser::Chrome, &program).to_json());
        return ExitCode::SUCCESS;
    }
    let code = match args.as_slice() {
        [flag, code] if flag == "--fill" => code.to_str().map(String::from),
        _ => None,
    };
    let endpoint =
        endpoint().unwrap_or_else(|| Endpoint::for_user().expect("a place for the socket"));
    let _listener = Listener::bind(endpoint, "0.1.0-stand-in", move |event| match event {
        Event::Linked(link) => println!("linked {} {:?}", link.id(), link.caller()),
        Event::Unlinked(link) => println!("unlinked {}", link.id()),
        Event::Message(link, message) => {
            println!(
                "{} {}",
                link.id(),
                serde_json::to_string(&message).unwrap_or_default()
            );
            if let (
                FromExtension::Tab {
                    tab_id,
                    url,
                    has_code_field: true,
                    ..
                },
                Some(code),
            ) = (&message, &code)
            {
                let fill = ToExtension::Fill {
                    tab_id: *tab_id,
                    url: url.clone(),
                    code: code.clone(),
                };
                if let Err(e) = link.send(&fill) {
                    println!("not sent: {e}");
                }
            }
        }
    })
    .expect("listening");
    println!("listening");
    loop {
        std::thread::park();
    }
}

#[cfg(unix)]
fn endpoint() -> Option<Endpoint> {
    std::env::var_os("AUTHWING_BRIDGE_SOCKET").map(|path| Endpoint::socket(path.into()))
}

#[cfg(windows)]
fn endpoint() -> Option<Endpoint> {
    std::env::var("AUTHWING_BRIDGE_SOCKET")
        .ok()
        .map(Endpoint::pipe)
}
