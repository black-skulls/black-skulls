use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::Result;
use superchain_client::futures;

/// A stream of OHLCV candles
pub struct VolatilityStream<P> {
    /// A stream of quotes
    price_stream: P,
    /// The list of old prices
    prices: VecDeque<f64>,
    /// The amount of prices we use for calculating volatility
    duration: usize,
}

impl<P> VolatilityStream<P> {
    /// Create a new volatility stream from a price stream
    pub fn new(price_stream: P, duration: usize) -> Self {
        Self {
            price_stream,
            prices: VecDeque::new(),
            duration,
        }
    }

    /// Handle the next price quote
    fn try_handle_price(&mut self, price: f64) -> Option<f64> {
        if price.is_infinite() || price.is_nan() {
            return None;
        }

        self.prices.push_back(price);
        if self.prices.len() < self.duration {
            return None;
        }

        let mut sum: f64 = 0.0;
        let mean: f64 = self.prices.iter().sum::<f64>() / self.prices.len() as f64;
        for i in 0..self.prices.len() {
            sum += (self.prices[i] - mean).powi(2);
        }
        let variance: f64 = sum / (self.prices.len() as f64);
        let volatility: f64 = variance.sqrt();

        Some(volatility)
    }
}

impl<Q> futures::Stream for VolatilityStream<Q>
where
    Q: futures::Stream<Item = Result<f64>> + Unpin,
{
    type Item = Result<f64>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let price = match Pin::new(&mut self.price_stream).poll_next(cx) {
            Poll::Ready(Some(Ok(price))) => price,
            Poll::Ready(Some(Err(err))) => return Poll::Ready(Some(Err(err))),
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Pending => return Poll::Pending,
        };

        match self.try_handle_price(price) {
            Some(volatility) => Poll::Ready(Some(Ok(volatility))),
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
