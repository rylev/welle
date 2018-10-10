extern crate reqwest;
extern crate tokio;
extern crate futures;

use tokio::prelude::*;
use futures::Future;
use reqwest::r#async::Client;

fn run(url: &str, n: usize, c: usize) -> impl Future<Item=(), Error=()> {
    let client = Client::new();

    let mut futures: Vec<Box<Future<Item = Result<(), reqwest::Error>, Error = ()> + Send>> = Vec::new();
    for x in 0 .. c {
        let mut requests = Vec::new();
        for y in 0 .. n {
            requests.push(client.get(url).send());
        }
        let future = join_all_serial(requests).then(move |result| {
            Ok(result.map(|_| ()))
        });
        futures.push(Box::new(future));
    }

    future::join_all(futures).map(|_| ())
}

fn main() {
    tokio::run(run("http://localhost:3000/echo", 5, 10))
}

#[derive(Debug)]
enum ElemState<T> where T: Future {
    Pending(T),
    Done(T::Item),
}
struct JoinAllSerial<I>
    where I: IntoIterator,
          I::Item: IntoFuture {
    elems: Vec<ElemState<<I::Item as IntoFuture>::Future>>
}

fn join_all_serial<I>(futures: I) -> JoinAllSerial<I>
    where I: IntoIterator,
          I::Item : IntoFuture
{
    let elems = futures.into_iter().map(|f| {
        ElemState::Pending(f.into_future())
    }).collect();
    JoinAllSerial { elems: elems }
}

impl<I> Future for JoinAllSerial<I>
    where I: IntoIterator,
          I::Item: IntoFuture,
{
    type Item = Vec<<I::Item as IntoFuture>::Item>;
    type Error = <I::Item as IntoFuture>::Error;


    fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
        let mut all_done = false;

        for idx in 0 .. self.elems.len() {
            let value = match self.elems[idx] {
                ElemState::Pending(ref mut t) => {
                    match t.poll() {
                        Ok(Async::Ready(v)) => {
                            if idx == self.elems.len() - 1 {
                                all_done = true;
                            }
                            Ok(v)
                        },
                        Ok(Async::NotReady) => { break },
                        Err(e) => Err(e),
                    }
                },
                _                        => continue
            };

            match value {
                Ok(v) => self.elems[idx] = ElemState::Done(v),
                Err(e) => {
                    // On completion drop all our associated resources ASAP.
                    self.elems = Vec::new();
                    return Err(e)
                }
            }
        }

        if all_done {
            let elems = std::mem::replace(&mut self.elems, Vec::new());
            let result = elems.into_iter().map(|e| {
                match e {
                    ElemState::Done(t) => t,
                    _ => unreachable!(),
                }
            }).collect();
            Ok(Async::Ready(result))
        } else {
            Ok(Async::NotReady)
        }
    }
}
