use std::time::{Duration, SystemTime};

use anyhow::Result;
use superchain_client::{
    config,
    ethers::types::H160,
    futures::{self, pin_mut, Stream, StreamExt, TryStreamExt},
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
    _priced: T,
    value: f64,
}

async fn timestamp<Q: Stream<Item = Price> + Unpin>(price_stream: Q, output: &mut Vec<f64>) -> () {
    let output2 = price_stream
        .map(|p| p.timestamp as f64)
        .collect::<Vec<_>>()
        .await;
    *output = output2;
}

async fn volatility<Q: Stream<Item = Price> + Unpin>(
    price_stream: Q,
    memory: u32,
    output: &mut Vec<f64>,
) -> () {
    let vol_stream = VolatilityStream::new(price_stream, memory);
    futures::pin_mut!(vol_stream);
    let output2 = vol_stream.map(|v| v.value).collect::<Vec<_>>().await;
    *output = output2;
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

    let (tx, rx) = async_channel::unbounded();

    let mut timestamps = Vec::<f64>::new();
    let mut vol50 = Vec::<f64>::new();
    let mut vol500 = Vec::<f64>::new();
    let mut vol5000 = Vec::<f64>::new();
    tokio_scoped::scope(|s| {
        s.spawn(timestamp(rx.clone(), &mut timestamps));
        s.spawn(volatility(rx.clone(), 50, &mut vol50));
        s.spawn(volatility(rx.clone(), 500, &mut vol500));
        s.spawn(volatility(rx.clone(), 5000, &mut vol5000));
        s.spawn(async move {
            prices
                .filter_map(|p_res| async { p_res.ok() })
                .for_each(|p| async { tx.send(p).await.unwrap() })
                .await
        });
    });

    let data50: Vec<(f64, f64)> = timestamps
        .iter()
        .cloned()
        .zip(vol50.iter().cloned())
        .collect();
    let data500: Vec<(f64, f64)> = timestamps
        .iter()
        .cloned()
        .zip(vol500.iter().cloned())
        .collect();
    let data5000: Vec<(f64, f64)> = timestamps
        .iter()
        .cloned()
        .zip(vol5000.iter().cloned())
        .collect();

    let line_chart50 = plotlib::repr::Plot::new(data50).line_style(
        plotlib::style::LineStyle::new()
            .colour("black")
            .linejoin(plotlib::style::LineJoin::Round),
    );
    let line_chart500 = plotlib::repr::Plot::new(data500).line_style(
        plotlib::style::LineStyle::new()
            .colour("black")
            .linejoin(plotlib::style::LineJoin::Round),
    );
    let line_chart5000 = plotlib::repr::Plot::new(data5000).line_style(
        plotlib::style::LineStyle::new()
            .colour("black")
            .linejoin(plotlib::style::LineJoin::Round),
    );
    let view = plotlib::view::ContinuousView::new()
        .add(line_chart50)
        .add(line_chart500)
        .add(line_chart5000);
    plotlib::page::Page::single(&view)
        .save("vol.svg")
        .expect("msg");
    Ok(())
}
