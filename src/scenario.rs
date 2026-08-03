//! The situations the scanner has to get right, made reproducible.
//!
//! Every scenario here exists because some rule in `cm-core` or `cm-diff` claims
//! to handle it. A scenario is how that claim gets tested against a real browser
//! load instead of a hand-written fixture, which is the failure mode this
//! project is most exposed to: a scanner that works beautifully on invented
//! input.

use clap::ValueEnum;

/// What the shop serves this run.
///
/// `Baseline` is the shop as it normally is. Every other variant changes exactly
/// one thing, so the resulting diff has exactly one cause and the severity it
/// produces can be read off without argument.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Scenario {
    /// The shop as it normally is. A scan of this against an approved baseline
    /// must come back clean, including across a restart, which changes the
    /// bundle filename hash and every cache buster.
    Baseline,

    /// The analytics vendor shipped a build. Routine, and the single most common
    /// real change: expect Medium, and expect nobody to be woken up.
    VendorUpdate,

    /// The shop's own checkout bundle changed. Expect High.
    FirstPartyChange,

    /// A script appears from an origin that has never been seen on this page.
    /// The British Airways and Ticketmaster shape. Expect Critical, and expect
    /// it to be the first line of the report.
    NewOrigin,

    /// Content-Security-Policy and X-Frame-Options are gone from the document
    /// response. Expect Critical.
    HeaderWeakened,

    /// The payment provider's SDK no longer loads. Rarely an attack, but the
    /// approved inventory is now wrong. Expect Medium.
    ScriptGone,

    /// A consent banner with no accept control the driver can find. The scanner
    /// must record an anomaly and label the capture pre-consent, never capture a
    /// pre-consent page and call it post-consent.
    UnhandleableConsent,

    /// The tag manager's dynamically inserted inline script carries a
    /// timestamp, so its content hash differs on every single load.
    ///
    /// Not a bug in the shop: `dataLayer.push({ ts: Date.now() })` is what half
    /// the tag managers on the internet emit. It is an open problem in
    /// normalisation, and this scenario is here so it can be reproduced on
    /// demand rather than discovered on a customer's site in week three.
    NoisyInline,

    /// `robots.txt` disallows `/checkout`. The scan must refuse before it loads
    /// anything, which is the one conduct rule that cannot be proven without a
    /// server that says no.
    RobotsDeny,
}

impl Scenario {
    pub fn as_str(self) -> &'static str {
        match self {
            Scenario::Baseline => "baseline",
            Scenario::VendorUpdate => "vendor-update",
            Scenario::FirstPartyChange => "first-party-change",
            Scenario::NewOrigin => "new-origin",
            Scenario::HeaderWeakened => "header-weakened",
            Scenario::ScriptGone => "script-gone",
            Scenario::UnhandleableConsent => "unhandleable-consent",
            Scenario::NoisyInline => "noisy-inline",
            Scenario::RobotsDeny => "robots-deny",
        }
    }

    /// What a scan of this scenario should produce, in one line. Printed at
    /// startup so the expected result is on screen next to the actual one.
    pub fn expectation(self) -> &'static str {
        match self {
            Scenario::Baseline => "clean against an approved baseline",
            Scenario::VendorUpdate => "one Medium: third-party script changed",
            Scenario::FirstPartyChange => "one High: your own script changed",
            Scenario::NewOrigin => "one Critical: script from a never-seen origin",
            Scenario::HeaderWeakened => "Critical: security header removed (CSP, X-Frame-Options)",
            Scenario::ScriptGone => "one Medium: approved script no longer loads",
            Scenario::UnhandleableConsent => "capture succeeds, anomaly recorded, consent state honest",
            Scenario::NoisyInline => "a Medium every run, forever: the open normalisation problem",
            Scenario::RobotsDeny => "refused by conduct before any load",
        }
    }
}

/// Which consent platform's markup the banner imitates.
///
/// Three real ones, because the accept control is per-platform and a heuristic
/// that guesses at it is the kind of cleverness that silently captures a
/// pre-consent page and labels it post-consent. The selectors here are the same
/// ones `cm-scan::consent` looks for, and that duplication is the test.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Cmp {
    /// Cookiebot by Usercentrics. Common on NL shops.
    Cookiebot,
    /// OneTrust. Common on larger merchants.
    Onetrust,
    /// CookieYes. Common on WooCommerce.
    Cookieyes,
    /// Usercentrics, rendered into a **shadow root**, which is how it actually
    /// ships. The accept control is in our platform table and a plain
    /// `document.querySelector` cannot reach it.
    ///
    /// Found on the first live scan of a real Dutch shop: the driver saw no
    /// banner, concluded there was nothing to accept, and labelled a
    /// pre-consent capture as post-consent with no anomaly. That is the one
    /// outcome cm-scan::consent exists to make impossible.
    Usercentrics,

    /// No banner at all: some shops load everything unconditionally.
    None,
}

impl Cmp {
    pub fn as_str(self) -> &'static str {
        match self {
            Cmp::Cookiebot => "cookiebot",
            Cmp::Onetrust => "onetrust",
            Cmp::Cookieyes => "cookieyes",
            Cmp::Usercentrics => "usercentrics-shadow-dom",
            Cmp::None => "none",
        }
    }
}
