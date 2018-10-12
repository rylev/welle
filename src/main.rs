extern crate reqwest;
extern crate tokio;
extern crate futures;
extern crate clap;

use clap::{Arg, App};
use tokio::prelude::*;
use futures::{Future,stream};
use reqwest::r#async::Client;

fn main() {
    let matches =
        App::new("Welle")
        .version("0.1")
        .author("Ryan Levick <ryan.levick@gmail.com>")
        .about("Load testing for servers")
        .arg(Arg::with_name("request_count")
             .short("n")
             .long("request-count")
             .value_name("NUMBER")
             .required(true)
             .help("Total number of requests to make")
             .takes_value(true))
        .arg(Arg::with_name("concurrent-count")
             .short("c")
             .long("concurrent-count")
             .value_name("NUMBER")
             .help("Number of in flight requests allowed at a time.")
             .takes_value(true))
        .get_matches();

    let request_count = matches.value_of("request_count").unwrap().parse::<usize>().expect("concurrent-count not a number");
    let concurrent_count = matches.value_of("concurrent_count").unwrap_or("1").parse::<usize>().expect("concurrent-count not a number");

    tokio::run(run("http://localhost:3000/echo", request_count, concurrent_count))
}

fn run(url: &'static str, request_count: usize, concurrent_count: usize) -> impl Future<Item=(), Error=()> {
    let client = Client::new();

    let futures =
        (0 .. request_count)
        .into_iter()
        .map(move |_| make_request(&client, url));

    stream::iter_ok(futures)
        .buffer_unordered(concurrent_count)
        .collect()
        .inspect(move |rs| {
            let sum: u32 = rs.iter().map(|d| d.subsec_micros()).sum();
            println!("{}ms", (sum as f64 / request_count as f64) / 1000.0);
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
