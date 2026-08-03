//! The third-party origins.
//!
//! Four listeners on four ports: a port is part of an origin, and origins are
//! the unit the scanner reasons in. One shared port would collapse the
//! distinction under test.
//!
//! | Origin      | Plays the part of              | Loaded                      |
//! |-------------|--------------------------------|-----------------------------|
//! | `cdn`       | a tag manager                  | always, from the page       |
//! | `analytics` | an analytics vendor            | after consent, by the tag manager |
//! | `psp`       | the payment provider's SDK     | always, from the page       |
//! | `rogue`     | an origin nobody approved      | only in `new-origin`        |

use axum::extract::State;
use axum::response::Response;
use axum::routing::get;
use axum::Router;

use crate::scenario::Scenario;
use crate::shop::asset_response;
use crate::Shop;

const JS: &str = "application/javascript; charset=utf-8";

pub fn cdn_router(shop: Shop) -> Router {
    Router::new()
        .route("/tagmanager.js", get(tagmanager))
        .with_state(shop)
}

pub fn analytics_router(shop: Shop) -> Router {
    Router::new()
        .route("/v3/collect.js", get(collect))
        .with_state(shop)
}

pub fn psp_router(shop: Shop) -> Router {
    Router::new().route("/sdk/pay.js", get(pay_sdk)).with_state(shop)
}

pub fn rogue_router(shop: Shop) -> Router {
    Router::new()
        .route("/lib/analytics-helper.min.js", get(rogue))
        .with_state(shop)
}

/// The tag manager: the reason a DOM walk is not a capture.
///
/// Everything it loads is inserted after the initial parse, by script, so the
/// page's HTML source shows this file and nothing it brings with it. Hence the
/// driver subscribing to `Debugger.scriptParsed`.
async fn tagmanager(State(shop): State<Shop>) -> Response {
    let rogue_injection = if shop.scenario == Scenario::NewOrigin {
        // Named to look like it belongs, as real skimmers are. A person reading
        // a list of URLs misses it; a machine comparing origins does not.
        format!(
            r#"
    load("{rogue}/lib/analytics-helper.min.js");"#,
            rogue = shop.origin_rogue
        )
    } else {
        String::new()
    };

    // Deterministic by default so a clean run is clean. The timestamped variant
    // hashes differently on every load, so it produces a Medium forever; that
    // is the open normalisation problem, reproduced on demand.
    let inline_body = if shop.scenario == Scenario::NoisyInline {
        r#"'window.dataLayer.push({ event: "consent_granted", ts: ' + Date.now() + ' });'"#
    } else {
        r#"'window.dataLayer.push({ event: "consent_granted" });'"#
    };

    let body = format!(
        r#"// tagmanager.js: loads tags once consent has been given.
(function () {{
  "use strict";

  function load(src) {{
    var s = document.createElement("script");
    s.src = src + "?cb=" + Date.now();
    s.async = true;
    document.head.appendChild(s);
  }}

  function fire() {{
    load("{analytics}/v3/collect.js");{rogue_injection}

    // An inserted script with no URL at all: absent from the HTML source,
    // nothing to record as a src, and visible only via Debugger.scriptParsed.
    var inline = document.createElement("script");
    inline.textContent = {inline_body};
    document.head.appendChild(inline);
  }}

  function decide() {{
    // This file is loaded from <head>, so the banner check has to wait for
    // the body; querying straight away finds no dialog and fires every tag
    // before consent. The shadow-DOM host has to be checked too, or the
    // Usercentrics banner would be treated as no banner at all.
    if (!document.querySelector('[role="dialog"]') &&
        !document.getElementById("usercentrics-root")) {{
      fire();
      return;
    }}

    if (/(^|;\s*)vf-consent=all/.test(document.cookie)) {{
      fire();
    }} else {{
      window.addEventListener("vf-consent-accepted", fire, {{ once: true }});
    }}
  }}

  if (document.readyState === "loading") {{
    document.addEventListener("DOMContentLoaded", decide, {{ once: true }});
  }} else {{
    decide();
  }}
}})();
"#,
        analytics = shop.origin_analytics,
        rogue_injection = rogue_injection,
        inline_body = inline_body,
    );

    asset_response(JS, body)
}

/// The analytics vendor, which ships builds.
async fn collect(State(shop): State<Shop>) -> Response {
    let extra = if shop.scenario == Scenario::VendorUpdate {
        "\n  send(\"pageview\", { sr: screen.width + \"x\" + screen.height });"
    } else {
        ""
    };

    let body = format!(
        r#"// collect.js: the analytics vendor's tag.
(function () {{
  "use strict";

  function send(name, payload) {{
    if (!navigator.sendBeacon) return;
    try {{
      navigator.sendBeacon("/collect", JSON.stringify({{ e: name, p: payload }}));
    }} catch (err) {{ /* analytics never breaks the page */ }}
  }}

  send("pageview", {{ path: location.pathname }});{extra}
}})();
"#
    );

    asset_response(JS, body)
}

/// The payment provider's SDK. When this goes missing the approved inventory is
/// wrong; it is not an attack.
async fn pay_sdk() -> Response {
    asset_response(
        JS,
        r#"// pay.js: the payment provider's client SDK.
(function () {
  "use strict";

  window.VfPay = {
    version: "4.2.0",
    tokenise: function (fields) {
      // A stub: the scanner never fills or submits a form, so nothing calls it.
      return Promise.resolve({ token: "tok_test_stub", fields: Object.keys(fields) });
    }
  };
})();
"#
        .to_string(),
    )
}

/// The origin nobody approved.
///
/// Deliberately harmless: it reads no fields and sends nothing. What makes it
/// Critical is that it is there and unaccounted for, which is the argument
/// behind 6.4.3.
async fn rogue() -> Response {
    asset_response(
        JS,
        r#"// analytics-helper.min.js
!function(){"use strict";var e=document.getElementById("payment-form");e&&e.addEventListener("submit",function(){})}();
"#
        .to_string(),
    )
}
