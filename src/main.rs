use superchain_client::{
    config,
    ethers::types::H160,
    futures::{self, TryStreamExt, StreamExt},
    tokio_tungstenite::connect_async,
    tungstenite::{client::IntoClientRequest, http::header::AUTHORIZATION},
    WsClient,
};

use crate::{quote::Quote, candle_stream::{Candle, CandleStream}};

mod candle_stream;
mod quote;

const URL: &str = "wss://beta.superchain.app/websocket";
const USDC: H160 = H160([
    180, 225, 109, 1, 104, 229, 45, 53, 202, 205, 44, 97, 133, 180, 66, 129, 236, 40, 201, 220,
]);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut req = URL.into_client_request()?;
    let config = config::Config::from_env();
    req.headers_mut().append(
        AUTHORIZATION,
        config.get_basic_authorization_value().try_into()?,
    );

    let (websocket, _) = connect_async(req).await.unwrap();
    let client = WsClient::new(websocket).await;

    let quotes = client
        .get_prices([USDC], Some(15500000), Some(15600000))
        .await?
        .map_err(anyhow::Error::from)
        .and_then(|price| async move {
            Ok(Quote {
                timestamp: time::OffsetDateTime::from_unix_timestamp(price.timestamp)?,
                price: price.price,
                volume: price.volume0,
            })
        });
    futures::pin_mut!(quotes);
    let candles = CandleStream::new(quotes, time::Duration::minutes(10));
    futures::pin_mut!(candles);

    while let Some(candle) = candles.next().await {
        let candle = candle?;
        println!("{candle:?}");
    }

    Ok(())
}
