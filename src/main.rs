use superchain_client::{
    config,
    ethers::types::H160,
    futures::{self, StreamExt, TryStreamExt},
    tokio_tungstenite::connect_async,
    tungstenite::{client::IntoClientRequest, http::header::AUTHORIZATION},
    WsClient,
};

use crate::{
    black_schools::{call as black_schools_put, call_discount as black_schools_call},
    black_schools_stream::BlackSchoolsStream,
    candle_stream::{Candle, CandleStream},
    quote::Quote,
    volatility_stream::VolatilityStream,
};

mod black_schools;
mod black_schools_stream;
mod candle_stream;
mod quote;
mod volatility_stream;

const URL: &str = "wss://beta.superchain.app/websocket";
const USDC: H160 = H160([
    180, 225, 109, 1, 104, 229, 45, 53, 202, 205, 44, 97, 133, 180, 66, 129, 236, 40, 201, 220,
]);
const CANDLE_DURATION: time::Duration = time::Duration::minutes(10);
const VOLATILITY_DURATION: usize = 1000;
const STRIKE: f64 = 1.;
const DIVIDENDS: f64 = 0.;

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

    let prices = client
        .get_prices([USDC], Some(15500000), Some(15600000))
        .await?
        .map_err(anyhow::Error::from)
        .map_ok(|price| price.price);
    futures::pin_mut!(prices);
    let black_schools = BlackSchoolsStream::new(prices, VOLATILITY_DURATION, STRIKE, DIVIDENDS);
    futures::pin_mut!(black_schools);

    while let Some(options_price) = black_schools.next().await {
        let options_price = options_price?;
        println!("{options_price}");
    }

    Ok(())
}
