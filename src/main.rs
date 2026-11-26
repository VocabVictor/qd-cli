#[tokio::main(flavor = "current_thread")]
async fn main() {
    qudongctl::configure_console();
    if let Err(error) = qudongctl::run().await {
        eprintln!("错误: {error:#}");
        std::process::exit(1);
    }
}
