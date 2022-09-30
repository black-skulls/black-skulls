use std::{
    pin::Pin,
    task::{Context, Poll},
};

use anyhow::Result;
use superchain_client::futures;
use time::{Duration, OffsetDateTime};

use crate::Quote;

#[derive(Clone, Copy, Debug, Default)]
pub struct Candle {
    /// The open price of a period
    pub open: f64,
    /// The highest price within a period
    pub high: f64,
    /// The lowest price within a period
    pub low: f64,
    /// The close price of a period
    pub close: f64,
    /// The volume within the period
    pub volume: f64,
}

/// A stream of OHLCV candles
pub struct CandleStream<Q> {
    /// A stream of quotes
    quote_stream: Q,
    /// The current candle
    candle: Candle,
    /// The last quote
    last: Option<Quote>,
    /// The start time of the current candle
    candle_start: OffsetDateTime,
    /// The duration of candles
    candle_duration: Duration,
}

impl<Q> CandleStream<Q> {
    /// Creates a new [`CandleStream`] with the specified duration
    ///
    /// ### Important
    /// - The start and end time of candles is not guaranteed to align with any round points
    ///   in time, like the full minute or full hour.
    ///
    /// ### Panics
    /// - If the duration is negative
    pub fn new(quote_stream: Q, candle_duration: Duration) -> Self {
        assert!(
            candle_duration.is_positive(),
            "Cannot create a candle streamer with a negative candle duration",
        );

        Self {
            quote_stream,
            candle: Candle::default(),
            last: None,
            candle_start: OffsetDateTime::UNIX_EPOCH,
            candle_duration,
        }
    }

    /// Try to process the next [`Quote`]
    ///
    /// This will yield the next candle and start processing the next candle if this quote lays
    /// outside of the timespan of the currently processed candle. Otherwise nothing is returned.
    ///
    /// ### Errors
    /// - If the passed [`Quote`] is older than the last processed quote
    fn try_handle_quote(&mut self, quote: Quote) -> Result<Option<(Candle, OffsetDateTime)>> {
        let last = match self.last {
            Some(last) => {
                if quote.timestamp < last.timestamp {
                    anyhow::bail!("received old quote: latest: `{last:?}; new: {quote:?}");
                }

                last
            }
            None => {
                self.last = Some(quote);
                return Ok(None);
            }
        };

        let mut candle = None;

        let is_new_candle = self.candle_start + self.candle_duration <= quote.timestamp;
        if is_new_candle {
            last.close_candle(&mut self.candle);
            candle = Some((self.candle, self.candle_start));

            self.candle = quote.new_candle();

            let skipped_candles =
                ((quote.timestamp - self.candle_start) / self.candle_duration).floor();
            self.candle_start += self.candle_duration * skipped_candles;
        } else {
            quote.update_candle(&mut self.candle);
        }

        self.last = Some(quote);

        Ok(candle)
    }
}

impl<Q> futures::Stream for CandleStream<Q>
where
    Q: futures::Stream<Item = Result<Quote>> + Unpin,
{
    type Item = Result<(Candle, OffsetDateTime)>;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let quote = match Pin::new(&mut self.quote_stream).poll_next(cx) {
            Poll::Ready(Some(Ok(quote))) => quote,
            Poll::Ready(Some(Err(err))) => return Poll::Ready(Some(Err(err))),
            Poll::Ready(None) => return Poll::Ready(None),
            Poll::Pending => return Poll::Pending,
        };

        match self.try_handle_quote(quote) {
            Ok(Some(candle)) => Poll::Ready(Some(Ok(candle))),
            Ok(None) => {
                cx.waker().wake_by_ref();
                Poll::Pending
            }
            Err(err) => Poll::Ready(Some(Err(err))),
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.quote_stream.size_hint()
    }
}
