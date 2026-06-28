use dotenvy::dotenv;
use polyoxide_relay::{BuilderAccount, BuilderConfig, RelayClient};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();

    let pk = env::var("POLYMARKET_PRIVATE_KEY").expect("POLYMARKET_PRIVATE_KEY must be set");

    // Optional builder creds
    let builder_key = env::var("BUILDER_API_KEY").ok();
    let builder_secret = env::var("BUILDER_SECRET").ok();
    let builder_pass = env::var("BUILDER_PASS_PHRASE").ok();

    let builder_config = if let (Some(k), Some(s)) = (builder_key, builder_secret) {
        Some(BuilderConfig::new(k, s, builder_pass))
    } else {
        None
    };

    let client = RelayClient::default_builder()?
        .with_account(BuilderAccount::new(pk, builder_config)?)
        .build()?;

    println!("Signer address: {:?}", client.address());

    let safe = client.get_expected_safe()?;
    println!("Expected Safe: {:?}", safe);

    let nonce = client.get_nonce(client.address().unwrap()).await?;
    println!("Nonce: {}", nonce);

    match client.get_deployed(safe).await {
        Ok(deployed) => println!("Safe deployed: {}", deployed),
        Err(e) => println!("Error checking safe deployment: {}", e),
    }

    Ok(())
}
