extern crate reqwest;
extern crate tokio;
extern crate futures;

use tokio::prelude::*;
use futures::Future;
use futures::stream;
use reqwest::r#async::Client;

fn main() {
    tokio::run(run("http://localhost:3000/echo", 500, 10))
}

fn run(url: &'static str, n: usize, c: usize) -> impl Future<Item=(), Error=()> {
    let client = Client::new();

    let futures =
        (0 .. n)
        .into_iter()
        .map(move |_| make_request(&client, url));

    stream::iter_ok(futures)
        .buffer_unordered(c)
        .collect()
        .inspect(|rs| {
            let sum: u32 = rs.iter().map(|d| d.subsec_millis()).sum();
            println!("{}ms", sum as f64 / rs.len() as f64);
        })
        .map(|_| ())
}

fn make_request(client: &Client, url: &str) -> impl Future<Item = std::time::Duration, Error = ()> + Send {
    let request = client.get(url);

    timed(request.send())
        .map(move |(_, duration)| duration)
}

#[derive(Debug)]
enum FutureState {
    Unpolled,
    Polled(std::time::Instant)
}
struct TimedFuture<F: Future> {
    inner: F,
    state: FutureState
}

fn timed<F: Future>(future: F) -> TimedFuture<F> {
    TimedFuture {
        inner: future,
        state: FutureState::Unpolled,
    }
}

impl <F> Future for TimedFuture<F>
    where F: Future {
    type Item = (Result<F::Item, F::Error>, std::time::Duration);
    type Error = ();

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        let now = std::time::Instant::now();
        let result = self.inner.poll();


        let t1 = match self.state {
            FutureState::Unpolled => now,
            FutureState::Polled(t) => t,
        };


        match result {
            Ok(Async::Ready(v)) => Ok(Async::Ready((Ok(v), now - t1))),
            Err(e) => Ok(Async::Ready((Err(e), now - t1))),
            Ok(Async::NotReady) => {
                self.state = FutureState::Polled(t1);
                Ok(Async::NotReady)
            }
        }
    }
}
