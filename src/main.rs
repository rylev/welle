extern crate reqwest;
extern crate tokio;
extern crate futures;

use tokio::prelude::*;
use futures::Future;
use futures::stream;
use reqwest::r#async::Client;

fn run(url: &str, n: usize, c: usize) -> impl Future<Item=(), Error=()> {
    let client = Client::new();

    let futures =
        (0 .. n)
        .into_iter()
        .map(|_| Wrapper(make_request(&client, url)))
        .collect::<Vec<Wrapper<_>>>();

    stream::iter_ok(futures)
        .buffer_unordered(c)
        .collect()
        .map(|_| ())
}

fn make_request(client: &Client, url: &str) -> impl Future<Item = Result<(), reqwest::Error>, Error = ()> + Send {
    client.get(url).send().inspect(move |_| {
    }).then(move |result| {
        Ok(result.map(|_| ()))
    })
}

fn main() {
    tokio::run(run("http://localhost:3000/echo", 1000, 10))
}
//
// #[derive(Debug)]
// enum ElemState<T> where T: Future {
//     Pending(T),
//     Done(T::Item),
// }
//
// struct JoinAllRateLimited<I>
//     where I: IntoIterator,
//           I::Item: IntoFuture {
//     elems: Vec<ElemState<<I::Item as IntoFuture>::Future>>,
//     rate_limit: usize
// }
//
// fn join_all_rate_limited<I>(futures: I, rate_limit: usize) -> JoinAllRateLimited<I>
//     where I: IntoIterator,
//           I::Item : IntoFuture
// {
//     if rate_limit == 0 {
//         panic!("Rate limit of 0 supplied");
//     }
//     let elems =
//         futures
//         .into_iter()
//         .map(|f| ElemState::Pending(f.into_future()))
//         .collect();
//     JoinAllRateLimited { elems, rate_limit }
// }
//
// impl<I> Future for JoinAllRateLimited<I>
//     where I: IntoIterator,
//           I::Item: IntoFuture,
// {
//     type Item = Vec<<I::Item as IntoFuture>::Item>;
//     type Error = <I::Item as IntoFuture>::Error;
//
//
//     fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
//         let mut in_flight = 0;
//         for idx in 0 .. self.elems.len() {
//             if in_flight >= self.rate_limit {
//                 break
//             }
//             let value = match self.elems[idx] {
//                 ElemState::Pending(ref mut t) => {
//                     in_flight += 1;
//                     match t.poll() {
//                         Ok(Async::Ready(v)) => Ok(v),
//                         Ok(Async::NotReady) => { continue },
//                         Err(e) => Err(e),
//                     }
//                 },
//                 _                        => continue
//             };
//
//             match value {
//                 Ok(v) => self.elems[idx] = ElemState::Done(v),
//                 Err(e) => {
//                     // On completion drop all our associated resources ASAP.
//                     self.elems = Vec::new();
//                     return Err(e)
//                 }
//             }
//         }
//         println!("In flight: {}", in_flight);
//
//         let any_left = self.elems.iter().any(|elem| {
//             match elem {
//                 ElemState::Pending(_) => true,
//                 _                     => false
//             }
//         });
//
//         if !any_left {
//             let elems = std::mem::replace(&mut self.elems, Vec::new());
//             let result = elems.into_iter().map(|e| {
//                 match e {
//                     ElemState::Done(t) => t,
//                     _ => unreachable!(),
//                 }
//             }).collect();
//             Ok(Async::Ready(result))
//         } else {
//             Ok(Async::NotReady)
//         }
//     }
// }
//
// struct JoinAllSerial<I>
//     where I: IntoIterator,
//           I::Item: IntoFuture {
//     elems: Vec<ElemState<<I::Item as IntoFuture>::Future>>
// }
//
// fn join_all_serial<I>(futures: I) -> JoinAllSerial<I>
//     where I: IntoIterator,
//           I::Item : IntoFuture
// {
//     let elems = futures.into_iter().map(|f| {
//         ElemState::Pending(f.into_future())
//     }).collect();
//     JoinAllSerial { elems: elems }
// }
//
// impl<I> Future for JoinAllSerial<I>
//     where I: IntoIterator,
//           I::Item: IntoFuture,
// {
//     type Item = Vec<<I::Item as IntoFuture>::Item>;
//     type Error = <I::Item as IntoFuture>::Error;
//
//
//     fn poll(&mut self) -> Poll<Self::Item, Self::Error> {
//         let mut all_done = false;
//
//         for idx in 0 .. self.elems.len() {
//             let value = match self.elems[idx] {
//                 ElemState::Pending(ref mut t) => {
//                     match t.poll() {
//                         Ok(Async::Ready(v)) => {
//                             if idx == self.elems.len() - 1 {
//                                 all_done = true;
//                             }
//                             Ok(v)
//                         },
//                         Ok(Async::NotReady) => { break },
//                         Err(e) => Err(e),
//                     }
//                 },
//                 _                        => continue
//             };
//
//             match value {
//                 Ok(v) => self.elems[idx] = ElemState::Done(v),
//                 Err(e) => {
//                     // On completion drop all our associated resources ASAP.
//                     self.elems = Vec::new();
//                     return Err(e)
//                 }
//             }
//         }
//
//         if all_done {
//             let elems = std::mem::replace(&mut self.elems, Vec::new());
//             let result = elems.into_iter().map(|e| {
//                 match e {
//                     ElemState::Done(t) => t,
//                     _ => unreachable!(),
//                 }
//             }).collect();
//             Ok(Async::Ready(result))
//         } else {
//             Ok(Async::NotReady)
//         }
//     }
// }

struct Wrapper<T> (T);

impl <F> Future for Wrapper<F> where F: Future{
    type Item = F::Item;
    type Error = F::Error;

    fn poll(&mut self) -> Result<Async<Self::Item>, Self::Error> {
        self.0.poll()
    }
}
