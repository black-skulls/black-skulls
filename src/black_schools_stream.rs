use std::{
    collections::VecDeque,
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::Result;
use superchain_client::futures;

use crate::{black_schools_call, VolatilityStream};

/// A stream of black schools
pub struct BlackSchoolsStream<P> {
    /// A stream of volatility
    price_stream: P,
    /// The volatility calculator
    volatility: VolatilityStream<()>,
    /// The strike price of the option
    strike: f64,
    /// The dividends payed by the underlying
    discount: f64,
}

impl<P> BlackSchoolsStream<P> {
    /// Create a new volatility stream from a price stream
    pub fn new(price_stream: P, volatility_duration: usize, strike: f64, discount: f64) -> Self {
        Self {
            price_stream,
            volatility: VolatilityStream::new((), volatility_duration),
            strike,
            discount,
        }
    }

    /// Handle the next price quote
    pub fn try_handle_price(&mut self, price: f64) -> Option<f64> {
        let volatility = self.volatility.try_handle_price(price)?;
        let options_price = black_schools_call(price, self.strike, self.discount, volatility);

        Some(options_price)
    }
}

impl<Q> futures::Stream for BlackSchoolsStream<Q>
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
            Some(options_price) => Poll::Ready(Some(Ok(options_price))),
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
