use anyhow::Result;

#[tokio::main]
async fn main() -> Result<()> {
    palyra_browserd::run().await
}
