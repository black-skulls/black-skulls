use std::{
    pin::Pin,
    task::{Context, Poll},
};

use superchain_client::futures::Stream;

use crate::{Priced, Volatility};
/*
struct State {
    memory: u32,
    last_variance: f64,
    count: u32,
    sum_prices: f64,
}

impl State {
    fn new(memory: u32) -> Self {
        Self {
            memory,
            last_variance: 0.0,
            count: 0,
            sum_prices: 0.0,
        }
    }
}

async fn clean_price<P: Priced>(priced: &P) -> bool {
    let price = priced.price();
    !price.is_infinite() && !price.is_nan()
}

async fn handle_price<P: Priced>(state: &mut State, priced: P) -> Option<Volatility<P>> {
    let price = priced.price();

    // let nominator1 = f64::try_from(u32::from(state.memory - 1) * u32::from(state.count)).ok()?;
    // let denominator =
    //     f64::try_from(u32::from(state.memory + 1) * u32::from(state.count + 1)).ok()?;
    // let nominator2 = 4f64;
    let nominator1 = f64::try_from(u32::from(state.count)).unwrap();
    let denominator = f64::try_from(u32::from(state.count + 1)).unwrap();
    let nominator2 = 2f64;

    let variance =
        (nominator1 * state.last_variance + nominator2 * price * state.sum_prices) / denominator;

    state.sum_prices += price;
    state.count += 1;
    state.last_variance = variance;

    let vol = Volatility::<P> {
        priced,
        value: variance.sqrt(),
    };
    Some(vol)
}

pub fn volatility_stream<Q, P>(price_stream: Q, memory: u32) -> impl Stream<Item = P>
where
    Q: Stream<Item = P>,
    P: Priced,
{
    let init = State::new(memory);
    price_stream.filter(clean_price) /* .scan(init, handle_price)*/
}
*/
/// A stream of OHLCV candles
pub struct VolatilityStream<Q, P> {
    /// A stream of quotes
    price_stream: Q,
    /// The amount of prices we use for calculating volatility
    memory: u32,
    last_variance: f64,
    count: u32,
    sum_prices: f64,
    _p: std::marker::PhantomData<P>,
}

impl<Q, P: Priced> VolatilityStream<Q, P> {
    /// Create a new volatility stream from a price stream
    pub fn new(price_stream: Q, memory: u32) -> Self {
        Self {
            price_stream,
            memory,
            last_variance: 0.0,
            count: 0,
            sum_prices: 0.0,
            _p: Default::default(),
        }
    }

    /// Handle the next price quote
    pub fn try_handle_price(&mut self, priced: P) -> Option<Volatility<P>> {
        let price = priced.price();
        if price.is_infinite() || price.is_nan() {
            // plain old data cleaning
            return None;
        }

        let nominator1 = f64::try_from(u32::from(self.memory - 1) * u32::from(self.count)).ok()?;
        let denominator =
            f64::try_from(u32::from(self.memory + 1) * u32::from(self.count + 1)).ok()?;
        let nominator2 = 4f64;
        // let nominator1 = f64::try_from(u32::from(self.count)).ok()?;
        // let denominator = f64::try_from(u32::from(self.count + 1)).ok()?;
        // let nominator2 = 2f64;

        let variance =
            (nominator1 * self.last_variance + nominator2 * price * self.sum_prices) / denominator;

        self.sum_prices += price;
        self.count += 1;
        self.last_variance = variance;

        let vol = Volatility::<P> {
            _priced: priced,
            value: variance.sqrt(),
        };
        Some(vol)
    }
}

impl<Q, P> Stream for VolatilityStream<Q, P>
where
    Q: Stream<Item = P> + Unpin,
    P: Priced + Unpin,
{
    type Item = Volatility<P>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let priced = match Pin::new(&mut self.price_stream).poll_next(cx) {
            Poll::Ready(Some(priced)) => priced,
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Pending => return Poll::Pending,
        };

        match self.try_handle_price(priced) {
            Some(volatility) => Poll::Ready(Some(volatility)),
            None => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        (0, self.price_stream.size_hint().1)
    }
}
