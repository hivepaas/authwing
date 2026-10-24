//! Where the user is, and which codes belong there.
//!
//! Three parts, none with UI:
//!
//! - [`site`]: whether the page the browser extension reports is one of a
//!   code's sites, or looks like one without being it, a phishing page to
//!   type nothing into.
//! - [`window`]: the app and window the user is in.
//! - [`mod@rank`]: from both, and from what the user chose before, the codes to
//!   put forward first, each with why.

pub mod rank;
pub mod site;
pub mod window;

pub use rank::{Candidate, Code, Context, Key, Ranking, Reason, Rule, Warning, rank};
pub use site::{Page, Resemblance, Site, SiteError, Verdict};
pub use window::{Access, Window};
