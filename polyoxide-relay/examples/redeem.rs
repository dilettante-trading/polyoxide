use alloy::primitives::{hex, U256};
use dotenvy::dotenv;
use polyoxide_relay::{BuilderAccount, BuilderConfig, RelayClient, WalletType};
use std::env;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load .env from the root directory
    dotenv().ok();

    // Hardcoded inputs for this example
    let relay_url = "https://relayer-v2.polymarket.com";
    let chain_id: u64 = 137;
    let condition_id_hex = "0x1171bfba0ad9386688133910593527fe77ce5406a7ac2c9a3552ab5471c1ac51";
    // outcome_index: 0 = NO, 1 = YES
    let outcome_index: u32 = 1;
    // index_set is a bitmask: 1 << outcome_index (1 for NO, 2 for YES, 3 for both)
    let index_set = U256::from(1u64 << outcome_index);

    // Load credentials from env
    let pk = env::var("POLYMARKET_PRIVATE_KEY")
        .expect("POLYMARKET_PRIVATE_KEY must be set (e.g. POLYMARKET_PRIVATE_KEY=0x...)");
    let builder_key = env::var("BUILDER_API_KEY").expect("BUILDER_API_KEY must be set");
    let builder_secret = env::var("BUILDER_SECRET").expect("BUILDER_SECRET must be set");
    let builder_pass = env::var("BUILDER_PASS_PHRASE").ok();

    let builder_config = BuilderConfig::new(builder_key, builder_secret, builder_pass);
    let account = BuilderAccount::new(pk, Some(builder_config))?;

    println!("Initializing RelayClient...");
    let client = RelayClient::default_builder()?
        .url(relay_url)?
        .with_account(account)
        .wallet_type(WalletType::Proxy)
        .build()?;
    println!("RelayClient initialized.");

    println!("Signer address: {:?}", client.address());
    let safe = client.get_expected_safe()?;
    println!("Expected Safe: {:?}", safe);

    println!("Redemption request details:");
    println!("  relay_url={}", relay_url);
    println!("  chain_id={}", chain_id);
    println!("  condition_id_hex={}", condition_id_hex);
    println!("  outcome_index={}", outcome_index);
    println!("  index_set={}", index_set);

    let condition_id: [u8; 32] = hex::decode(&condition_id_hex[2..])?
        .try_into()
        .map_err(|_| "Invalid condition ID length")?;
    let index_sets = vec![index_set];

    println!("Attempting to redeem condition id: {}", condition_id_hex);

    match client
        .submit_gasless_redemption_with_gas_estimation(condition_id, index_sets, true)
        .await
    {
        Ok(response) => {
            println!("Redemption submitted successfully!");
            println!("Transaction ID: {}", response.transaction_id);
        }
        Err(e) => {
            eprintln!("Redemption failed: {:?}", e);
        }
    }

    Ok(())
}
