use crate::Candle;

#[derive(Clone, Copy, Debug)]
pub struct Quote {
    /// The time at which the quote was created
    pub timestamp: time::OffsetDateTime,
    /// The price of the quote
    pub price: f64,
    /// The volume of the quote
    pub volume: f64,
}

impl Quote {
    /// Creates a new candle from this quote
    ///
    /// This sets OHLV to the values of the quote and C to 0.
    pub fn new_candle(&self) -> Candle {
        Candle {
            open: self.price,
            high: self.price,
            low: self.price,
            volume: self.volume,
            ..Default::default()
        }
    }

    /// Updates a candle based on this quote
    ///
    /// This will adjust high or low if necessary and increase the volume
    /// by the volume of this quote.
    pub fn update_candle(&self, candle: &mut Candle) {
        candle.high = candle.high.max(self.price);
        candle.low = candle.low.min(self.price);
        candle.volume += self.volume;
    }

    /// Close a candle with this as the last quote
    ///
    /// This will update the quote using [`Quote::update_candle`] and
    /// set the closing price to this quote's price.
    pub fn close_candle(&self, candle: &mut Candle) {
        candle.close = self.price;
    }
}

impl Default for Quote {
    fn default() -> Self {
        Self {
            timestamp: time::OffsetDateTime::UNIX_EPOCH,
            price: 0.0,
            volume: 0.0,
        }
    }
}
