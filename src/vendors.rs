//! The third-party origins.
//!
//! Four separate listeners, on four ports, because a port is part of an origin
//! and origins are the unit the whole product reasons in. Serving all of this
//! from one port would collapse exactly the distinction the scanner exists to
//! make.
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
use crate::shop::script_like;
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
/// Everything it loads is inserted after the initial parse, by script. Reading
/// the page's HTML source finds this file and nothing it brings with it, which
/// is why `Debugger.scriptParsed` rather than a DOM query is the thing the
/// driver subscribes to.
async fn tagmanager(State(shop): State<Shop>) -> Response {
    let rogue_injection = if shop.scenario == Scenario::NewOrigin {
        // Named to look like something that belongs. Real skimmers are named
        // like this, and a person scanning a list of URLs does not catch it.
        // A machine comparing origins against an approved set does.
        format!(
            r#"
    load("{rogue}/lib/analytics-helper.min.js");"#,
            rogue = shop.origin_rogue
        )
    } else {
        String::new()
    };

    // The body of the dynamically inserted inline script.
    //
    // Deterministic by default so a clean run is actually clean. The timestamped
    // variant is a real and very common shape, and it is a genuinely open
    // problem: its content hash differs on every single load, so it produces a
    // Medium every day forever. `--scenario noisy-inline` reproduces it on
    // demand, because inventing a normalisation rule for it before it has been
    // seen on five real shops is exactly the speculation this project cannot
    // afford. See crawler/CHECKS.md.
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

    // A dynamically inserted script with no URL at all. This is the case that
    // separates a real capture from a plausible one: it never appears in the
    // HTML source, it has no src to record, and Debugger.scriptParsed is the
    // only place it shows up.
    var inline = document.createElement("script");
    inline.textContent = {inline_body};
    document.head.appendChild(inline);
  }}

  function decide() {{
    // The banner check has to wait for the body. This file is loaded from
    // <head>, so querying for the dialog straight away always finds nothing and
    // every tag fires before consent, which is both a real bug shops have and
    // the exact thing that makes a pre-consent capture worthless.
    // Also look for a shadow-DOM banner host, or this shop would fire its tags
    // before consent whenever the banner is one, which is not what real CMPs do.
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

    script_like(JS, body)
}

/// The analytics vendor. Ships builds; that is its whole personality.
async fn collect(State(shop): State<Shop>) -> Response {
    let extra = if shop.scenario == Scenario::VendorUpdate {
        // A vendor shipping a build is the most common change this product will
        // ever see. If it wakes anyone at night, the product is broken.
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

    script_like(JS, body)
}

/// The payment provider's SDK. Stable, boring, and the one that going missing
/// means the inventory is wrong rather than that anyone is under attack.
async fn pay_sdk() -> Response {
    script_like(
        JS,
        r#"// pay.js: the payment provider's client SDK.
(function () {
  "use strict";

  window.VfPay = {
    version: "4.2.0",
    tokenise: function (fields) {
      // A real SDK would post to the provider here. This one is a stub: the
      // scanner never fills a form and never submits one, so nothing calls it.
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
/// Deliberately harmless: it reads no fields and sends nothing. What makes it a
/// Critical finding is not what it does, it is that it is there and no one wrote
/// down why. That is the entire argument for 6.4.3, and for this product.
async fn rogue() -> Response {
    script_like(
        JS,
        r#"// analytics-helper.min.js
!function(){"use strict";var e=document.getElementById("payment-form");e&&e.addEventListener("submit",function(){})}();
"#
        .to_string(),
    )
}
