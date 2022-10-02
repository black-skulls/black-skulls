use superchain_client::futures::{Stream, StreamExt};

use crate::{Priced, Volatility};

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

pub fn volatility_stream<Q, P>(
    price_stream: Q,
    memory: u32,
) -> impl Stream<Item = Volatility<P>> + 'static
where
    Q: Stream<Item = P> + Unpin + 'static,
    P: Priced + 'static,
{
    let init = State::new(memory);
    price_stream.scan(init, |state, priced| {
        let price = priced.price();

        let nominator1 =
            f64::try_from(u32::from(state.memory - 1) * u32::from(state.count)).unwrap();
        let denominator =
            f64::try_from(u32::from(state.memory + 1) * u32::from(state.count + 1)).unwrap();
        let nominator2 = 4f64;

        let variance = (nominator1 * state.last_variance + nominator2 * price * state.sum_prices)
            / denominator;

        state.sum_prices += price;
        state.count += 1;
        state.last_variance = variance;

        let vol = Volatility::<P> {
            _priced: priced,
            value: variance.sqrt(),
        };
        std::future::ready(Some(vol))
    })
}
