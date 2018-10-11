extern crate reqwest;
extern crate tokio;
extern crate futures;

use tokio::prelude::*;
use futures::Future;
use futures::stream;
use reqwest::r#async::Client;

fn main() {
    tokio::run(run("http://localhost:3000/echo", 1000, 10))
}

fn run(url: &'static str, n: usize, c: usize) -> impl Future<Item=(), Error=()> {
    let client = Client::new();

    let futures =
        (0 .. n)
        .into_iter()
        .map(move |_| make_request(&client, url));

    stream::iter_ok(futures)
        .buffer_unordered(c)
        .fold((), |_, _| future::ok(()))
}

fn make_request(client: &Client, url: &str) -> impl Future<Item = Result<(), reqwest::Error>, Error = ()> + Send {
    client.get(url).send().inspect(|_| {
    }).then(|result| {
        Ok(result.map(|_| ()))
    })
}
