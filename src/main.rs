use std::time::{Duration, SystemTime};

use superchain_client::{
    config,
    ethers::types::H160,
    futures::{self, StreamExt, TryStreamExt},
    tokio_tungstenite::connect_async,
    tungstenite::{client::IntoClientRequest, http::header::AUTHORIZATION},
    Price, WsClient,
};

use crate::volatility_stream::VolatilityStream;

mod volatility_stream;

const URL: &str = "wss://beta.superchain.app/websocket";
const USDC: H160 = H160([
    180, 225, 109, 1, 104, 229, 45, 53, 202, 205, 44, 97, 133, 180, 66, 129, 236, 40, 201, 220,
]);

pub trait Priced {
    fn price(&self) -> f64;
}

impl Priced for Price {
    fn price(&self) -> f64 {
        self.price
    }
}

pub struct Volatility<T: Priced> {
    priced: T,
    value: f64,
}

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
        .map_err(anyhow::Error::from);
    let volatility = VolatilityStream::new(prices, 500);
    futures::pin_mut!(volatility);

    let data: Vec<(f64, f64)> = volatility
        //.skip(1000)
        .filter_map(|v_res| async { v_res.ok().map(|v| (v.priced.timestamp as f64, v.value)) })
        .collect()
        .await;
    // while let Some(point) = volatility.next().await {
    //     let point = point?;
    //     println!(
    //         "{},{}",
    //         humantime::format_rfc3339_seconds(
    //             SystemTime::UNIX_EPOCH
    //                 + Duration::from_secs(u64::try_from(point.priced.timestamp).unwrap())
    //         ),
    //         point.value
    //     );
    // }

    let line_chart = plotlib::repr::Plot::new(data).line_style(
        plotlib::style::LineStyle::new()
            .colour("black")
            .linejoin(plotlib::style::LineJoin::Round),
    );
    let view = plotlib::view::ContinuousView::new().add(line_chart);
    plotlib::page::Page::single(&view)
        .save("vol.svg")
        .expect("msg");

    Ok(())
}
