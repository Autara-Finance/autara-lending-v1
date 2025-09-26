use anyhow::Context;
use arch_program::bitcoin::{key::Keypair, Network};
use arch_sdk::{
    arch_program::{
        account::AccountMeta, instruction::Instruction, pubkey::Pubkey, sanitized::ArchMessage,
    },
    build_and_sign_transaction, AsyncArchRpcClient, Status,
};
use autara_lib::oracle::pyth::PythPrice;
use serde::{Deserialize, Serialize};

pub const BTC_FEED: &str = "0xe62df6c8b4a85fe1a67db44dc12de5db330f7ac66b72dc658afedf0f4a415b43";
pub const USDC_FEED: &str = "0xeaa020c61cc479712813461ce153894a96a6c00b21ed0cfc2798d1f9a9e9c94a";

pub async fn fetch_and_push_feeds(
    client: &AsyncArchRpcClient,
    autara_oracle_program_id: &Pubkey,
    signer: &Keypair,
    feeds: &[impl AsRef<str>],
    bitcoin_network: Network,
) {
    let signer_pubkey = Pubkey::from_slice(&signer.x_only_public_key().0.serialize());
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        let pyth_api_result = match fetch_pyth_price(feeds).await {
            Ok(ok) => ok,
            Err(err) => {
                tracing::error!("Failed to fetch Pyth price: {}", err);
                continue;
            }
        };
        let ixs = match pyth_api_result
            .parsed
            .into_iter()
            .map(|data| {
                tracing::info!(
                    "{} price = {}, ema = {}",
                    data.id,
                    data.price.as_float(),
                    data.ema_price.as_float()
                );
                let oracle_account: PythPrice = data.try_into()?;
                let pyth_account = get_pyth_account(autara_oracle_program_id, oracle_account.id);
                Ok(Instruction {
                    program_id: *autara_oracle_program_id,
                    accounts: vec![
                        AccountMeta::new(signer_pubkey, true),
                        AccountMeta::new(pyth_account, false),
                        AccountMeta::new_readonly(
                            arch_sdk::arch_program::system_program::SYSTEM_PROGRAM_ID,
                            false,
                        ),
                    ],
                    data: bytemuck::bytes_of(&oracle_account).to_vec(),
                })
            })
            .collect::<Result<Vec<_>, anyhow::Error>>()
        {
            Ok(ixs) => ixs,
            Err(err) => {
                tracing::error!("Failed to convert Pyth price data: {}", err);
                continue;
            }
        };
        if let Err(err) =
            build_and_send_tx(client, &signer_pubkey, signer, &ixs, bitcoin_network).await
        {
            tracing::error!("Failed to send transaction: {:?}", err);
        }
    }
}

pub struct AutaraPythPusherClient {
    pub client: AsyncArchRpcClient,
    pub autara_oracle_program_id: Pubkey,
    pub network: Network,
}

impl AutaraPythPusherClient {
    pub async fn push_pyth_price(
        &self,
        signer: &Keypair,
        pyth_feed_id: [u8; 32],
        oracle: &PythPrice,
    ) -> anyhow::Result<()> {
        let key = Pubkey(signer.x_only_public_key().0.serialize());
        let pyth_account = get_pyth_account(&self.autara_oracle_program_id, pyth_feed_id);
        let ixs = vec![Instruction {
            program_id: self.autara_oracle_program_id,
            accounts: vec![
                AccountMeta::new(key, true),
                AccountMeta::new(pyth_account, false),
                AccountMeta::new_readonly(
                    arch_sdk::arch_program::system_program::SYSTEM_PROGRAM_ID,
                    false,
                ),
            ],
            data: bytemuck::bytes_of(oracle).to_vec(),
        }];
        build_and_send_tx(&self.client, &key, signer, &ixs, self.network).await
    }
}

pub async fn build_and_send_tx(
    client: &AsyncArchRpcClient,
    signer_pk: &Pubkey,
    signer: &Keypair,
    ixs: &[Instruction],
    bitcoin_network: Network,
) -> anyhow::Result<()> {
    let message = ArchMessage::new(
        ixs,
        Some(*signer_pk),
        client.get_best_block_hash().await?.try_into()?,
    );
    let tx = build_and_sign_transaction(message, vec![*signer], bitcoin_network)?;
    let sig = hex::encode(&tx.signatures.first().context("no transaction ID")?.0);
    tracing::info!("Sending {sig:?}");
    let txids = client.send_transactions(vec![tx]).await?;
    let processed_txs = client.wait_for_processed_transactions(txids).await?;
    let result = processed_txs.first().context("no transactions processed")?;
    if result.status != Status::Processed {
        let msg = format!(
            "Transaction failed {sig} with status = {:?} and logs = {:?}",
            result.status, result.logs
        );
        tracing::error!("{msg}");
        return Err(anyhow::anyhow!(msg));
    }
    Ok(())
}

pub async fn fetch_pyth_price(
    ids: impl IntoIterator<Item = impl AsRef<str>>,
) -> anyhow::Result<Root> {
    const PYTH_URL: &str = "https://hermes.pyth.network/v2/updates/price/latest?";
    let ids: Vec<String> = ids
        .into_iter()
        .map(|id| format!("ids[]={}", id.as_ref()))
        .collect();
    let url = format!("{}{}", PYTH_URL, ids.join("&"));
    Ok(reqwest::get(url).await?.json::<_>().await?)
}

pub fn get_pyth_account(autara_oracle_program_id: &Pubkey, pyth_feed_id: [u8; 32]) -> Pubkey {
    Pubkey::find_program_address(&[&pyth_feed_id], autara_oracle_program_id).0
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Root {
    pub binary: Binary,
    pub parsed: Vec<ParsedPrice>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Binary {
    pub encoding: String,
    pub data: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ParsedPrice {
    pub id: String,
    pub price: PriceData,
    pub ema_price: PriceData,
    pub metadata: Metadata,
}

impl TryInto<PythPrice> for ParsedPrice {
    type Error = anyhow::Error;
    fn try_into(self) -> Result<PythPrice, Self::Error> {
        let mut id: [u8; 32] = [0; 32];
        hex::decode_to_slice(&self.id, &mut id)?;
        Ok(PythPrice {
            id,
            price: self.price.into(),
            ema_price: self.ema_price.into(),
            metadata: self.metadata.into(),
        })
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PriceData {
    #[serde(deserialize_with = "de_string_to_u64")]
    pub price: u64,
    #[serde(deserialize_with = "de_string_to_u64")]
    pub conf: u64,
    pub expo: i32,
    pub publish_time: i64,
}

impl PriceData {
    pub fn as_float(&self) -> f64 {
        self.price as f64 * 10f64.powi(self.expo)
    }
}

impl Into<autara_lib::oracle::pyth::PriceData> for PriceData {
    fn into(self) -> autara_lib::oracle::pyth::PriceData {
        autara_lib::oracle::pyth::PriceData {
            price: self.price,
            conf: self.conf,
            expo: self.expo as i64,
            publish_time: self.publish_time,
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Metadata {
    pub slot: u64,
    pub proof_available_time: i64,
    pub prev_publish_time: i64,
}

impl Into<autara_lib::oracle::pyth::Metadata> for Metadata {
    fn into(self) -> autara_lib::oracle::pyth::Metadata {
        autara_lib::oracle::pyth::Metadata {
            slot: self.slot,
            proof_available_time: self.proof_available_time,
            prev_publish_time: self.prev_publish_time,
        }
    }
}

fn de_string_to_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    s.parse().map_err(serde::de::Error::custom)
}
