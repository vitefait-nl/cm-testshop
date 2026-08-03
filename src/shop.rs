//! The merchant's own origin: the checkout page, its bundle, and `robots.txt`.
//!
//! The page is built to look like an ordinary Dutch guest checkout that collects
//! card details on its own domain, because that is the shape the product is sold
//! against: SAQ A-EP or D, where 6.4.3 and 11.6.1 actually apply. A shop that
//! redirects to the provider's hosted page is out of scope and is the thing
//! `ProspectFinding::qualifies` disqualifies.

use axum::body::Body;
use axum::extract::State;
use axum::http::{header, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use crate::scenario::{Cmp, Scenario};
use crate::Shop;

pub fn router(shop: Shop) -> Router {
    Router::new()
        .route("/", get(home))
        .route("/checkout", get(checkout))
        .route("/product", get(product))
        .route("/admin", get(admin))
        .route("/robots.txt", get(robots))
        .route("/assets/{file}", get(asset))
        .with_state(shop)
}

/// A landing page, so `/checkout` is reached the way a shopper reaches it: by a
/// link. The conduct rules forbid requesting a URL that was not linked from the
/// page under test, and a scanner developed against a shop with no link in it
/// would never exercise that.
async fn home(State(shop): State<Shop>) -> Response {
    let body = format!(
        r#"<!doctype html>
<html lang="nl">
<head><meta charset="utf-8"><title>Testwinkel</title></head>
<body>
  <h1>Testwinkel</h1>
  <p>Een neppe winkel, alleen om de scanner tegen te ontwikkelen.</p>
  <p>Scenario van deze run: <code>{scenario}</code></p>
  <p><a href="/checkout">Naar de kassa</a></p>
</body>
</html>"#,
        scenario = shop.scenario.as_str()
    );
    html(body, Vec::new())
}

/// Disallowed in `robots.txt` in every scenario. Nothing here matters; its only
/// job is to exist at a path the crawler must never fetch.
async fn admin() -> Response {
    html(
        "<!doctype html><html lang=\"nl\"><body><h1>Beheer</h1></body></html>".to_string(),
        Vec::new(),
    )
}

async fn robots(State(shop): State<Shop>) -> Response {
    // `/admin` is always disallowed. The `robots-deny` scenario additionally
    // disallows the checkout itself, which is the case that proves the conduct
    // gate refuses before anything loads.
    let text = match shop.scenario {
        Scenario::RobotsDeny => {
            "User-agent: *\nDisallow: /admin\nDisallow: /checkout\n\nSitemap: /sitemap.xml\n"
        }
        _ => "User-agent: *\nDisallow: /admin\nAllow: /\n\nSitemap: /sitemap.xml\n",
    };

    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        text,
    )
        .into_response()
}

/// A product page with an add-to-basket control.
///
/// Only reachable in the `needs-basket` scenario, and named by the target rather
/// than found: the scanner is not allowed to go looking for it.
async fn product(State(shop): State<Shop>) -> Response {
    let body = format!(
        r#"<!doctype html>
<html lang="nl">
<head><meta charset="utf-8"><title>Voorbeeldproduct | Testwinkel</title></head>
<body>
  <h1>Voorbeeldproduct</h1>
  <p>EUR 99,00</p>
  <form id="product-form" method="post" action="/cart/add">
    <label for="qty">Aantal</label>
    <input id="qty" name="qty" value="1">
    <!-- WooCommerce's control, because that is the one the table lists first.
         The scanner clicks this and types nothing, including in the field above,
         which is there precisely so that "nothing is typed" is testable. -->
    <button class="single_add_to_cart_button" type="button">In winkelwagen</button>
    <button class="newsletter-signup" type="button">Aanmelden nieuwsbrief</button>
  </form>
  <script>
    document.querySelector(".single_add_to_cart_button")
      .addEventListener("click", function () {{
        document.cookie = "vf-basket=1; path=/; max-age=3600";
        document.body.insertAdjacentHTML("beforeend", "<p id=\"added\">Toegevoegd.</p>");
      }});
  </script>
</body>
</html>"#
    );
    let _ = shop;
    html(body, Vec::new())
}

/// The page under test.
async fn checkout(
    State(shop): State<Shop>,
    headers: HeaderMap,
) -> Response {
    // The empty-basket page. Most real checkouts do this: without a basket there
    // is nothing to pay for, so no payment page is rendered.
    if shop.scenario == Scenario::NeedsBasket {
        let has_basket = headers
            .get(header::COOKIE)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|c| c.contains("vf-basket=1"));
        if !has_basket {
            return html(
                r#"<!doctype html>
<html lang="nl">
<head><meta charset="utf-8"><title>Winkelwagen leeg | Testwinkel</title></head>
<body>
  <h1>Je winkelwagen is leeg</h1>
  <p>Voeg eerst een product toe.</p>
  <p><a href="/product">Naar het voorbeeldproduct</a></p>
</body>
</html>"#
                    .to_string(),
                Vec::new(),
            );
        }
    }

    // Changes on every request. Nothing about the script it points at changes,
    // so a scanner that reports this is reporting noise, and `normalise_url`
    // exists precisely to make it not do that.
    let cache_buster = crate::now_secs().to_string();

    let psp_tag = if shop.scenario == Scenario::ScriptGone {
        // The vendor's SDK is simply absent this run. The approved inventory is
        // now wrong, which is a Medium and worth saying out loud.
        String::new()
    } else {
        format!(
            r#"  <script src="{psp}/sdk/pay.js" defer></script>"#,
            psp = shop.origin_psp
        )
    };

    let body = format!(
        r#"<!doctype html>
<html lang="nl">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Afrekenen | Testwinkel</title>
  <link rel="stylesheet" href="/assets/checkout.{build}.css">

  <script>
    // An inline script with no URL of its own. A capture built from a DOM walk
    // at the end of the load can still see this one; the dynamically inserted
    // inline script further down is the one that needs Debugger.scriptParsed.
    window.dataLayer = window.dataLayer || [];
    window.dataLayer.push({{ event: "begin_checkout", currency: "EUR", value: 99.00 }});
  </script>

  <script src="{cdn}/tagmanager.js?id=GTM-NL7QX2&_={cb}"></script>
{psp_tag}
</head>
<body>
{banner}

  <main>
    <h1>Afrekenen</h1>

    <section>
      <h2>Je bestelling</h2>
      <p>1 x Voorbeeldproduct, EUR 99,00</p>
    </section>

    <!-- Card fields on the merchant's own origin. This is what puts a shop in
         SAQ A-EP or D, and it is the signal ProspectFinding looks for. The
         scanner reads this form and never fills or submits it. -->
    <form id="payment-form" method="post" action="/checkout/confirm">
      <h2>Betaalgegevens</h2>
      <label for="card-number">Kaartnummer</label>
      <input id="card-number" name="card-number" autocomplete="cc-number" inputmode="numeric">

      <label for="card-expiry">Vervaldatum</label>
      <input id="card-expiry" name="card-expiry" autocomplete="cc-exp" placeholder="MM/JJ">

      <label for="card-cvc">CVC</label>
      <input id="card-cvc" name="card-cvc" autocomplete="cc-csc" inputmode="numeric">

      <button type="submit">Betaal EUR 99,00</button>
    </form>
  </main>

  <script src="/assets/checkout.{build}.js?v={cb}"></script>
</body>
</html>"#,
        build = shop.build_id,
        cdn = shop.origin_cdn,
        cb = cache_buster,
        psp_tag = psp_tag,
        banner = banner_markup(&shop),
    );

    html(body, security_headers(&shop))
}

/// First-party assets.
///
/// The filename carries a content hash that changes on every restart of this
/// binary, which is what a deploy looks like from outside. The body behind it
/// does not change unless the scenario says so. A scanner that alerts on the
/// filename is alerting on a deploy; `normalise::group_key` is what stops it.
async fn asset(
    State(shop): State<Shop>,
    axum::extract::Path(file): axum::extract::Path<String>,
) -> Response {
    if file.ends_with(".css") {
        return script_like(
            "text/css; charset=utf-8",
            ":root{font-family:system-ui,sans-serif}main{max-width:34rem;margin:2rem auto}\
             input{display:block;width:100%;padding:.5rem;margin:.25rem 0 1rem}"
                .to_string(),
        );
    }

    if !file.ends_with(".js") {
        return StatusCode::NOT_FOUND.into_response();
    }

    script_like("application/javascript; charset=utf-8", checkout_js(&shop))
}

/// The shop's own checkout logic.
fn checkout_js(shop: &Shop) -> String {
    let extra = if shop.scenario == Scenario::FirstPartyChange {
        // One added line, the size of a real change: enough to move the content
        // hash, not enough to be obvious to a person reading a diff. That is the
        // point. A first-party change is High because the shop's own code is
        // where an attacker who got in would put the skimmer.
        "\n  form.dataset.vfRevision = \"2\";"
    } else {
        ""
    };

    format!(
        r#"// checkout.js: validation and totals for the guest checkout.
(function () {{
  "use strict";

  var form = document.getElementById("payment-form");
  if (!form) return;

  function digitsOnly(el) {{
    el.addEventListener("input", function () {{
      el.value = el.value.replace(/[^0-9 ]/g, "");
    }});
  }}

  digitsOnly(document.getElementById("card-number"));
  digitsOnly(document.getElementById("card-cvc"));

  form.addEventListener("submit", function (event) {{
    var number = document.getElementById("card-number").value.replace(/\s+/g, "");
    if (number.length < 12) {{
      event.preventDefault();
      window.alert("Controleer je kaartnummer.");
    }}
  }});{extra}
}})();
"#
    )
}

/// The consent banner, in the markup of whichever platform was asked for.
///
/// The accept control's id or class is the real one each platform ships, because
/// that is what `cm-scan::consent` matches on. Imitating the platform loosely
/// would make this a test of nothing.
fn banner_markup(shop: &Shop) -> String {
    let accept_control = match shop.cmp {
        Cmp::None => return String::new(),

        Cmp::Cookiebot => {
            r#"<div id="CybotCookiebotDialog" role="dialog" aria-label="Cookietoestemming">
    <p>Wij gebruiken cookies om de winkel te laten werken en om te meten hoe hij gebruikt wordt.</p>
    <button id="CybotCookiebotDialogBodyLevelButtonLevelOptinAllowAll" type="button">Alles toestaan</button>
    <button id="CybotCookiebotDialogBodyButtonDecline" type="button">Alleen noodzakelijke</button>
  </div>"#
        }

        Cmp::Onetrust => {
            r#"<div id="onetrust-banner-sdk" role="dialog" aria-label="Cookietoestemming">
    <p>Wij gebruiken cookies om de winkel te laten werken en om te meten hoe hij gebruikt wordt.</p>
    <button id="onetrust-accept-btn-handler" type="button">Alle cookies accepteren</button>
    <button id="onetrust-reject-all-handler" type="button">Alles weigeren</button>
  </div>"#
        }

        // Rendered into a shadow root by the script below, not written here:
        // the whole point is that the accept control is not in the light DOM.
        Cmp::Usercentrics => {
            return format!(
                r#"<div id="usercentrics-root"></div>
  <script>
    (function () {{
      var host = document.getElementById("usercentrics-root");
      var root = host.attachShadow({{ mode: "open" }});
      // The markup a real Dutch shop served on 2 August 2026, shape and ids
      // preserved. The accept control is `#accept.uc-accept-button` and the
      // decline control is `#deny.uc-deny-button`, which is why the table
      // matches on the class rather than the bare id.
      root.innerHTML =
        '<div role="dialog" aria-label="Cookietoestemming">' +
        '<div id="uc-cmp-description" class="overflow"><p>Wij gebruiken cookies.</p></div>' +
        '<a id="uc-more-link" class="uc-button-link" href="/cookie-instellingen">Instellingen beheren</a>' +
        '<button id="deny" class="deny uc-deny-button" type="button">weigeren</button>' +
        '<button id="accept" class="accept uc-accept-button" type="button">akkoord</button>' +
        '</div>';
      root.querySelector('button.uc-accept-button')
          .addEventListener("click", function () {{
        document.cookie = "vf-consent=all; path=/; max-age=86400";
        host.style.display = "none";
        window.dispatchEvent(new CustomEvent("vf-consent-accepted"));
      }});
    }})();
  </script>"#
            );
        }

        Cmp::Cookieyes => {
            r#"<div class="cky-consent-container" role="dialog" aria-label="Cookietoestemming">
    <p>Wij gebruiken cookies om de winkel te laten werken en om te meten hoe hij gebruikt wordt.</p>
    <button class="cky-btn cky-btn-accept" type="button">Accepteren</button>
    <button class="cky-btn cky-btn-reject" type="button">Weigeren</button>
  </div>"#
        }
    };

    // The unhandleable case: a house-built banner whose accept control carries
    // no recognisable id, class or accessible name. It is not contrived; plenty
    // of shops roll their own. The scanner must notice it cannot proceed and say
    // so, rather than capturing the pre-consent page and calling it post-consent.
    let markup = if shop.scenario == Scenario::UnhandleableConsent {
        r#"<div class="c-x9f2" role="dialog">
    <p>Wij gebruiken cookies.</p>
    <span class="c-x9f2__a" tabindex="0">Ga verder</span>
  </div>"#
            .to_string()
    } else {
        accept_control.to_string()
    };

    format!(
        r#"{markup}
  <script>
    // Consent state lives in one place and is announced once. The tag manager
    // waits for this event; nothing gated is loaded before it fires.
    (function () {{
      var banner = document.querySelector('[role="dialog"]');
      if (!banner) return;
      function accept() {{
        document.cookie = "vf-consent=all; path=/; max-age=86400";
        banner.style.display = "none";
        window.dispatchEvent(new CustomEvent("vf-consent-accepted"));
      }}
      banner.querySelectorAll("button").forEach(function (b) {{
        if (/toestaan|accepteren|accepteer/i.test(b.textContent)) {{
          b.addEventListener("click", accept);
        }}
      }});
    }})();
  </script>"#
    )
}

/// The security-relevant response headers on the main document.
///
/// Only the main document's headers are recorded by the scanner; subresource
/// headers are noise. `header-weakened` removes two of them, which is the shape
/// of a real regression: a CSP dropped during a deploy, nobody notices, and the
/// page is one injected tag away from a bad afternoon.
fn security_headers(shop: &Shop) -> Vec<(&'static str, String)> {
    let mut headers = vec![
        (
            "strict-transport-security",
            "max-age=31536000; includeSubDomains".to_string(),
        ),
        ("x-content-type-options", "nosniff".to_string()),
        (
            "referrer-policy",
            "strict-origin-when-cross-origin".to_string(),
        ),
        (
            "permissions-policy",
            "geolocation=(), microphone=(), camera=()".to_string(),
        ),
        (
            "set-cookie",
            "vf-session=testshop; Path=/; HttpOnly; SameSite=Lax".to_string(),
        ),
    ];

    if shop.scenario != Scenario::HeaderWeakened {
        // The `new-origin` scenario permits the rogue origin as well.
        //
        // Not a concession: without it the browser blocks the injected script,
        // it never executes, and the scanner correctly reports nothing, which
        // makes the scenario a test of Chromium's CSP rather than of this
        // product. A shop whose CSP already permits the origin an attacker
        // reaches is the ordinary case in the wild: most NL checkouts carry no
        // script-src at all, and those that do usually list a wildcard or a tag
        // manager that can load anything it likes.
        let permitted = if shop.scenario == Scenario::NewOrigin {
            format!(
                "{cdn} {analytics} {psp} {rogue}",
                cdn = shop.origin_cdn,
                analytics = shop.origin_analytics,
                psp = shop.origin_psp,
                rogue = shop.origin_rogue,
            )
        } else {
            format!(
                "{cdn} {analytics} {psp}",
                cdn = shop.origin_cdn,
                analytics = shop.origin_analytics,
                psp = shop.origin_psp,
            )
        };

        headers.push((
            "content-security-policy",
            format!(
                "default-src 'self'; script-src 'self' 'unsafe-inline' {permitted}; \
                 style-src 'self' 'unsafe-inline'; frame-ancestors 'none'"
            ),
        ));
        headers.push(("x-frame-options", "DENY".to_string()));
    }

    headers
}

fn html(body: String, extra: Vec<(&'static str, String)>) -> Response {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/html; charset=utf-8"),
    );
    // No caching anywhere in this shop: a capture must be of what the server
    // says now, not of what a proxy remembered.
    headers.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, must-revalidate"),
    );

    for (name, value) in extra {
        if let (Ok(n), Ok(v)) = (
            header::HeaderName::from_bytes(name.as_bytes()),
            HeaderValue::from_str(&value),
        ) {
            headers.append(n, v);
        }
    }

    (StatusCode::OK, headers, Body::from(body)).into_response()
}

pub fn script_like(content_type: &'static str, body: String) -> Response {
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-store, must-revalidate"),
            // Third-party scripts are cross-origin by construction here.
            (header::ACCESS_CONTROL_ALLOW_ORIGIN, "*"),
        ],
        Body::from(body),
    )
        .into_response()
}
