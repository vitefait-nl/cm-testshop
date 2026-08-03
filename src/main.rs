//! `cm-testshop`: a fake Dutch guest checkout, served from five local origins.
//!
//! The one target the scanner may do anything to, and the source of every
//! capture taken before Gate 1. It deliberately serves the hard cases: a tag
//! manager that inserts its children after the parse (including an inline
//! script with no URL), a bundle filename whose hash changes every restart,
//! per-request cache busters, three real consent platforms plus one banner that
//! cannot be handled, removable security headers, and a `robots.txt` that can
//! say no.
//!
//! ```sh
//! cargo run
//! cargo run -- --scenario new-origin
//! cargo run -- --cmp onetrust
//! ```

mod scenario;
mod shop;
mod vendors;

use std::time::{SystemTime, UNIX_EPOCH};

use axum::Router;
use clap::Parser;
use sha2::{Digest, Sha256};
use tokio::net::TcpListener;

use crate::scenario::{Cmp, Scenario};

#[derive(Parser)]
#[command(name = "cm-testshop", about = "A fake checkout for developing the scanner")]
struct Cli {
    /// First-party port. The four vendor origins take the next four.
    #[arg(long, default_value_t = 8081)]
    port: u16,

    /// Which situation to serve. `--help` lists what each one should produce.
    #[arg(long, value_enum, default_value_t = Scenario::Baseline)]
    scenario: Scenario,

    /// Which consent platform's markup the banner imitates.
    #[arg(long, value_enum, default_value_t = Cmp::Cookiebot)]
    cmp: Cmp,

    /// Pin the bundle's content hash instead of deriving a fresh one per start.
    /// Use when a test needs two runs to be byte-identical.
    #[arg(long)]
    build_id: Option<String>,
}

#[derive(Clone)]
pub struct Shop {
    pub scenario: Scenario,
    pub cmp: Cmp,
    pub build_id: String,
    pub origin_cdn: String,
    pub origin_analytics: String,
    pub origin_psp: String,
    pub origin_rogue: String,
}

pub fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("cm_testshop=info")),
        )
        .with_target(false)
        .init();

    let build_id = cli.build_id.unwrap_or_else(|| {
        // Eight hex characters, the shape a bundler emits. From the start time,
        // so it changes per run and is reproducible within one.
        let digest = Sha256::digest(now_secs().to_le_bytes());
        hex::encode(&digest[..4])
    });

    let base = cli.port;
    let origin = |offset: u16| format!("http://127.0.0.1:{}", base + offset);

    let shop = Shop {
        scenario: cli.scenario,
        cmp: cli.cmp,
        build_id,
        origin_cdn: origin(1),
        origin_analytics: origin(2),
        origin_psp: origin(3),
        origin_rogue: origin(4),
    };

    println!("cm-testshop");
    println!("  scenario   {}  ({})", shop.scenario.as_str(), shop.scenario.expectation());
    println!("  consent    {}", shop.cmp.as_str());
    println!("  build id   {}", shop.build_id);
    println!();
    println!("  checkout   {}/checkout", origin(0));
    println!("  robots     {}/robots.txt", origin(0));
    println!("  tagmanager {}", shop.origin_cdn);
    println!("  analytics  {}", shop.origin_analytics);
    println!("  psp        {}", shop.origin_psp);
    println!("  rogue      {}  (loaded only in --scenario new-origin)", shop.origin_rogue);
    println!();
    println!("  scan it:");
    println!(
        "    cargo run -p cm-cli -- --root ./work scan --url {}/checkout --id my-test-shop",
        origin(0)
    );

    let servers = vec![
        serve(base, shop::router(shop.clone())),
        serve(base + 1, vendors::cdn_router(shop.clone())),
        serve(base + 2, vendors::analytics_router(shop.clone())),
        serve(base + 3, vendors::psp_router(shop.clone())),
        serve(base + 4, vendors::rogue_router(shop.clone())),
    ];

    // Any listener failing takes the whole shop down: a capture missing one
    // origin reads as a false Medium.
    for handle in servers {
        handle.await??;
    }

    Ok(())
}

fn serve(port: u16, router: Router) -> tokio::task::JoinHandle<Result<(), std::io::Error>> {
    tokio::spawn(async move {
        let listener = TcpListener::bind(("127.0.0.1", port)).await?;
        tracing::info!(port, "listening");
        axum::serve(listener, router).await
    })
}
