//! The merchant's own origin: the checkout page, its bundle, and `robots.txt`.
//!
//! Card fields are collected on this origin rather than in a hosted redirect,
//! because that is the shape the product is sold against (SAQ A-EP or D, where
//! 6.4.3 and 11.6.1 apply) and the shape `ProspectFinding::qualifies` accepts.

use axum::body::Body;
use axum::extract::State;
use axum::extract::Path;
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
        .route("/cart/add/{id}", get(cart_add))
        .route("/checkouts/cn/{token}", get(token_checkout))
        .route("/admin", get(admin))
        .route("/robots.txt", get(robots))
        .route("/assets/{file}", get(asset))
        .with_state(shop)
}

/// A landing page, so `/checkout` is reached by a link. The conduct rules
/// forbid requesting a URL that was not linked from the page under test, and
/// that cannot be exercised against a shop with no links.
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

/// Disallowed in `robots.txt` in every scenario: a path the crawler must never
/// fetch. The content is irrelevant.
async fn admin() -> Response {
    html(
        "<!doctype html><html lang=\"nl\"><body><h1>Beheer</h1></body></html>".to_string(),
        Vec::new(),
    )
}

async fn robots(State(shop): State<Shop>) -> Response {
    // `/admin` is always disallowed; `robots-deny` disallows the checkout too,
    // which is what proves the conduct gate refuses before anything loads.
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

/// A product page carrying an add-to-basket control, for click priming.
/// Reachable only in `needs-basket`; the scanner is given its URL, never
/// allowed to go looking for it.
async fn product(State(shop): State<Shop>) -> Response {
    let _ = shop;
    html(
        r#"<!doctype html>
<html lang="nl">
<head><meta charset="utf-8"><title>Voorbeeldproduct | Testwinkel</title></head>
<body>
  <h1>Voorbeeldproduct</h1>
  <p>EUR 99</p>
  <form id="product-form" method="post" action="/cart/add/1">
    <label for="qty">Aantal</label>
    <input id="qty" name="qty" value="1">
    <!-- The scanner clicks this and types nothing, including in the field
         above, which is there so "nothing is typed" is testable. -->
    <button class="single_add_to_cart_button" type="button">In winkelwagen</button>
    <button class="newsletter-signup" type="button">Aanmelden nieuwsbrief</button>
  </form>
  <script>
    document.querySelector(".single_add_to_cart_button")
      .addEventListener("click", function () {
        document.cookie = "vf-basket=1; path=/; max-age=3600";
        document.body.insertAdjacentHTML("beforeend", "<p id=\"added\">Toegevoegd.</p>");
      });
  </script>
</body>
</html>"#
            .to_string(),
        Vec::new(),
    )
}

/// The link-based shape: visiting the URL fills the basket, no control to
/// click. Plenty of Dutch shops expose exactly this.
async fn cart_add(State(shop): State<Shop>, Path(id): Path<String>) -> Response {
    let _ = shop;
    let body = format!(
        r#"<!doctype html>
<html lang="nl">
<head><meta charset="utf-8"><title>Toegevoegd | Testwinkel</title></head>
<body>
  <h1>Toegevoegd aan je winkelwagen</h1>
  <p>Artikel {id} staat in je winkelwagen.</p>
  <p><a href="/checkout">Naar de kassa</a></p>
</body>
</html>"#
    );
    html(
        body,
        vec![(
            "set-cookie",
            "vf-basket=1; Path=/; Max-Age=3600; SameSite=Lax".to_string(),
        )],
    )
}

/// The basket page of a shop whose checkout has no stable URL: the control
/// leads to a token URL minted per visit, as Shopify does.
async fn token_basket() -> Response {
    let token: String = format!("{:x}", crate::now_secs().wrapping_mul(2_654_435_761));
    let body = format!(
        r#"<!doctype html>
<html lang="nl">
<head><meta charset="utf-8"><title>Winkelwagen | Testwinkel</title></head>
<body>
  <h1>Je winkelwagen</h1>
  <p>Voorbeeldproduct, EUR 99</p>
  <button name="checkout" onclick="location.href='/checkouts/cn/{token}'">Afrekenen</button>
</body>
</html>"#
    );
    html(body, Vec::new())
}

/// The checkout behind that token. Same page every run; only its URL moves.
async fn token_checkout(State(shop): State<Shop>, Path(token): Path<String>) -> Response {
    let _ = token;
    checkout_page(shop).await
}

/// The page under test.
async fn checkout(State(shop): State<Shop>, headers: HeaderMap) -> Response {
    if shop.scenario == Scenario::TokenCheckout {
        return token_basket().await;
    }

    // Most real checkouts do this: with nothing in the basket there is nothing
    // to pay for, so no payment page is rendered.
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

    checkout_page(shop).await
}

/// The checkout itself, whatever URL it was reached by.
async fn checkout_page(shop: Shop) -> Response {
    // Changes every request while the script behind it does not, which is the
    // noise `normalise_url` exists to strip.
    let cache_buster = crate::now_secs().to_string();

    let psp_tag = if shop.scenario == Scenario::ScriptGone {
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
    // An inline script with no URL, but present in the source: a DOM walk
    // finds this one. The tag manager's inserted inline script does not.
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

    <!-- The scanner reads this form and never fills or submits it. -->
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
/// The filename carries a content hash that changes on every restart, which is
/// what a deploy looks like from outside; the body behind it changes only when
/// the scenario says so. `normalise::group_key` is what keeps the rename from
/// reading as a finding.
async fn asset(
    State(shop): State<Shop>,
    axum::extract::Path(file): axum::extract::Path<String>,
) -> Response {
    if file.ends_with(".css") {
        return asset_response(
            "text/css; charset=utf-8",
            ":root{font-family:system-ui,sans-serif}main{max-width:34rem;margin:2rem auto}\
             input{display:block;width:100%;padding:.5rem;margin:.25rem 0 1rem}"
                .to_string(),
        );
    }

    if !file.ends_with(".js") {
        return StatusCode::NOT_FOUND.into_response();
    }

    asset_response("application/javascript; charset=utf-8", checkout_js(&shop))
}

/// The shop's own checkout logic.
fn checkout_js(shop: &Shop) -> String {
    let extra = if shop.scenario == Scenario::FirstPartyChange {
        // One line: enough to move the content hash, not enough for a person
        // reading the diff to catch.
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
/// Every accept control's id and class is the real one that platform ships,
/// because that is what `cm-scan::consent` matches on. Approximating the markup
/// would make this a test of nothing.
fn banner_markup(shop: &Shop) -> String {
    let platform_markup = match shop.cmp {
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

        // Rendered into a shadow root by the script below rather than written
        // here: the point is that the accept control is not in the light DOM.
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

    // A house-built banner whose accept control carries no recognisable id,
    // class or accessible name. The scanner must say it cannot proceed rather
    // than capture the pre-consent page and call it post-consent.
    let markup = if shop.scenario == Scenario::UnhandleableConsent {
        r#"<div class="c-x9f2" role="dialog">
    <p>Wij gebruiken cookies.</p>
    <span class="c-x9f2__a" tabindex="0">Ga verder</span>
  </div>"#
            .to_string()
    } else {
        platform_markup.to_string()
    };

    format!(
        r#"{markup}
  <script>
    // The tag manager waits for this event; nothing gated loads before it.
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

/// The security-relevant response headers on the main document. The scanner
/// records only these; subresource headers are noise.
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
        // `new-origin` must permit the rogue origin, or Chromium blocks the
        // injected script and the scenario tests the browser's CSP rather than
        // this product. It is also the ordinary case: most NL checkouts carry
        // no script-src, and those that do list a wildcard or a tag manager
        // that can load anything.
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

        // The nonce differs on every response, as real shops serve it: two
        // shops fired a Critical per run on nothing else (5 Aug 2026). It
        // lives in the report-only header so it cannot disable
        // 'unsafe-inline' for the shop's own inline scripts, and every matrix
        // row re-proves the value is noise while its presence stays content.
        let nonce = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .subsec_nanos();
        headers.push((
            "content-security-policy-report-only",
            format!("script-src 'self' 'nonce-{nonce}' {permitted}"),
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
    // No caching anywhere: a capture must be of what the server says now.
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

pub fn asset_response(content_type: &'static str, body: String) -> Response {
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
