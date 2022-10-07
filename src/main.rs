mod volatility_stream;

use plotlib::repr::ContinuousRepresentation;
use superchain_client::{
    config,
    ethers::types::H160,
    futures::{self, Stream, StreamExt, TryStreamExt},
    tokio_tungstenite::connect_async,
    tungstenite::{client::IntoClientRequest, http::header::AUTHORIZATION},
    Price, WsClient,
};

use volatility_stream::volatility_stream;

const URL: &str = "wss://beta.superchain.app/websocket";
const USDC: H160 = H160([
    180, 225, 109, 1, 104, 229, 45, 53, 202, 205, 44, 97, 133, 180, 66, 129, 236, 40, 201, 220,
]);
const FROM_BLOCK: Option<u64> = Some(15500000);
const TO_BLOCK_INC: Option<u64> = Some(15600000);
const FILENAME: &str = "vol.svg";

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

async fn price<Q: Stream<Item = Price> + Unpin + 'static>(
    price_stream: Q,
    output: &mut Vec<f64>,
) -> () {
    // We are used to WETH in USDC prices, not the other way around
    let output2 = price_stream
        .map(|p| 1.0 / p.price)
        .collect::<Vec<_>>()
        .await;
    *output = output2;
}

async fn volatility<Q: Stream<Item = Price> + Unpin + 'static>(
    price_stream: Q,
    memory: u32,
    output: &mut Vec<f64>,
) -> () {
    let vol_stream = volatility_stream(price_stream, memory);
    futures::pin_mut!(vol_stream);
    let output2 = vol_stream.map(|v| v.value).collect::<Vec<_>>().await;
    *output = output2;
}

fn into_data<'t>(
    timestamps: impl IntoIterator<Item = &'t f64>,
    values: impl IntoIterator<Item = f64>,
) -> Vec<(f64, f64)> {
    const EVERY: usize = 10;
    timestamps
        .into_iter()
        .step_by(EVERY)
        .cloned()
        .zip(values.into_iter().step_by(EVERY))
        .collect()
}

fn into_chart(
    data: Vec<(f64, f64)>,
    colour: impl Into<String>,
) -> impl ContinuousRepresentation + 'static {
    plotlib::repr::Plot::new(data).line_style(
        plotlib::style::LineStyle::new()
            .colour(colour)
            .linejoin(plotlib::style::LineJoin::Round),
    )
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
        .get_prices([USDC], FROM_BLOCK, TO_BLOCK_INC)
        .await?
        .map_err(anyhow::Error::from);

    let (tx, rx) = async_channel::unbounded();

    let mut timestamps = Vec::<f64>::new();
    let mut plain_prices = Vec::<f64>::new();
    let mut vol50 = Vec::<f64>::new();
    let mut vol500 = Vec::<f64>::new();
    let mut vol5000 = Vec::<f64>::new();
    tokio_scoped::scope(|s| {
        s.spawn(timestamp(rx.clone(), &mut timestamps));
        s.spawn(price(rx.clone(), &mut plain_prices));
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

    println!("{} swaps received and processed", timestamps.len());

    let max_price = plain_prices.iter().map(|p| *p as u64).max().unwrap() as f64;
    println!("The max price is {max_price}");

    let data_price = into_data(&timestamps, plain_prices);
    let data50 = into_data(&timestamps, vol50);
    let data500 = into_data(&timestamps, vol500);
    let data5000 = into_data(&timestamps, vol5000);

    const QUANTIZER: f64 = 1_000_000.0;
    let max_vol = data50
        .iter()
        .map(|(_t, v)| (*v * QUANTIZER) as u64)
        .max()
        .unwrap() as f64
        / QUANTIZER;
    println!("Max volatility: {max_vol}");

    let line_chart_price = into_chart(data_price, "black");
    let line_chart50 = into_chart(data50, "red");
    let line_chart500 = into_chart(data500, "orange");
    let line_chart5000 = into_chart(data5000, "green");

    let min_x = timestamps.iter().map(|t| *t as u64).min().unwrap() as f64;
    let max_x = timestamps.iter().map(|t| *t as u64).max().unwrap() as f64;
    // would be nice to have the y axis on the right...
    let price_view = plotlib::view::ContinuousView::new()
        .x_range(min_x, max_x)
        .y_range(0.0, max_price)
        .add(line_chart_price);
    let vol_view = plotlib::view::ContinuousView::new()
        .x_range(min_x, max_x)
        .y_range(0.0, max_vol)
        .add(line_chart50)
        .add(line_chart500)
        .add(line_chart5000);

    plotlib::page::Page::empty()
        .dimensions(1920, 1080)
        .add_plot(&price_view)
        .add_plot(&vol_view)
        .save(FILENAME)
        .expect("msg");

    println!("Written {FILENAME} to disk");

    Ok(())
}
