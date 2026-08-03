//! The situations the scanner has to get right, made reproducible.
//!
//! Every scenario exists because some rule in `cm-core` or `cm-diff` claims to
//! handle it, and a scenario is how that claim gets tested against a real
//! browser load rather than a hand-written fixture.

use clap::ValueEnum;

/// What the shop serves this run. Every variant but `Baseline` changes exactly
/// one thing, so the resulting diff has exactly one cause.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Scenario {
    /// The shop as it normally is. Must scan clean against an approved
    /// baseline, including across a restart, which changes the bundle filename
    /// hash and every cache buster.
    Baseline,

    /// The analytics vendor shipped a build. The most common real change:
    /// expect Medium, and expect nobody to be woken up.
    VendorUpdate,

    /// The shop's own checkout bundle changed. Expect High.
    FirstPartyChange,

    /// A script appears from an origin never seen on this page: the British
    /// Airways and Ticketmaster shape. Expect Critical, first line of the
    /// report.
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

    /// The tag manager's inserted inline script carries a timestamp, so its
    /// content hash differs on every load. Not a bug in the shop:
    /// `dataLayer.push({ ts: Date.now() })` is what half the tag managers on
    /// the internet emit. Normalising it is an open problem; this reproduces it
    /// on demand.
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

    /// What a scan of this scenario should produce. Printed at startup so the
    /// expected result is on screen next to the actual one.
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
/// The selectors here are the same ones `cm-scan::consent` looks for. That
/// duplication is the test: keep them byte-accurate to the real platforms.
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
#[value(rename_all = "kebab-case")]
pub enum Cmp {
    /// Cookiebot by Usercentrics. Common on NL shops.
    Cookiebot,
    /// OneTrust. Common on larger merchants.
    Onetrust,
    /// CookieYes. Common on WooCommerce.
    Cookieyes,
    /// Usercentrics, rendered into a shadow root, which is how it ships. A
    /// plain `document.querySelector` cannot reach the accept control.
    ///
    /// From the first live scan of a real Dutch shop: the driver saw no banner,
    /// concluded there was nothing to accept, and labelled a pre-consent
    /// capture as post-consent with no anomaly.
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
