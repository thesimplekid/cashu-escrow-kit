use std::time::Duration;

use crate::model::EscrowRegistration;
use anyhow::anyhow;
use nostr_sdk::prelude::*;
use tokio::time::timeout;

pub struct NostrClient {
    keys: Keys,
    pub client: Client,
}

impl NostrClient {
    pub async fn new(keys: Keys) -> anyhow::Result<Self> {
        let client = Client::new(&keys);

        client.add_relay("wss://relay.damus.io").await?;
        client.add_relay("wss://relay.primal.net").await?;
        client.add_relay("wss://relay.nostr.band").await?;
        client
            .add_relay("wss://ftp.halifax.rwth-aachen.de/nostr")
            .await?;
        client.add_relay("wss://nostr.mom").await?;
        //client.add_relay("wss://relay.nostrplebs.com").await?; (having errors)

        // Connect to relays
        client.connect().await;
        Ok(Self { keys, client })
    }

    pub fn public_key(&self) -> PublicKey {
        self.keys.public_key()
    }

    pub async fn receive_escrow_message(&self, timeout_secs: u64) -> anyhow::Result<String> {
        let message_filter = Filter::new()
            .kind(Kind::GiftWrap)
            .pubkey(self.keys.public_key())
            .limit(0);

        let subscription_id = self.client.subscribe(vec![message_filter], None).await?.val;

        let mut notifications = self.client.notifications();

        let loop_future = async {
            loop {
                if let Ok(notification) = notifications.recv().await {
                    if let RelayPoolNotification::Event { event, .. } = notification {
                        let rumor = self.client.unwrap_gift_wrap(&event).await?.rumor;
                        if rumor.kind == Kind::PrivateDirectMessage {
                            break Ok(rumor.content) as anyhow::Result<String>;
                        }
                    }
                }
            }
        };
        let result = match timeout(Duration::from_secs(timeout_secs), loop_future).await {
            Ok(result) => result,
            Err(e) => Err(anyhow!("Timeout, {}", e)),
        };
        self.client.unsubscribe(subscription_id.clone()).await;

        result
    }

    // coordinator specific function?
    pub async fn send_escrow_registration(
        &self,
        receivers: (PublicKey, PublicKey),
        id: &[u8; 32],
        trade_pk: &str,
    ) -> anyhow::Result<()> {
        let registration_json = serde_json::to_string(&EscrowRegistration {
            escrow_id_hex: hex::encode(id),
            coordinator_escrow_pubkey: cdk::nuts::PublicKey::from_hex(trade_pk)?,
            escrow_start_time: Timestamp::now(),
        })?;
        // todo: replace deprecated method
        self.client
            .send_private_msg(receivers.0, &registration_json, None)
            .await?;
        self.client
            .send_private_msg(receivers.1, &registration_json, None)
            .await?;
        Ok(())
    }
}
