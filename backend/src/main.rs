// @spec APP-HEALTH-003
#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // `showrunner-backend --healthcheck` is the image's HEALTHCHECK command:
    // probe the running server over loopback and exit 0/1. Nothing else is
    // loaded so the probe can only fail for the server's reasons.
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args == ["--healthcheck"] {
        let ok = showrunner_backend::api::health::probe_from_env().await;
        std::process::exit(if ok { 0 } else { 1 });
    }

    showrunner_backend::run().await
}
