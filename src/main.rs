use futures::{stream, Future};
use http::HttpTryFrom;
use reqwest::{r#async::Client, Method, Url};
use std::time::Duration;
use structopt::StructOpt;
use tokio::prelude::*;

/// A tool for load testing servers
#[derive(StructOpt, Debug)]
#[structopt(name = "welle")]
struct Config {
    /// Total number of requests to make
    #[structopt(
        short = "n",
        long = "num-requests",
        value_name = "NUMBER",
        required = true,
        raw(validator = "parse_number_of_requests")
    )]
    request_count: usize,

    /// Number of in flight requests allowed at a time
    #[structopt(
        short = "c",
        long = "concurrent-requests",
        value_name = "NUMBER",
        default_value = "1"
    )]
    concurrent_count: usize,

    /// HTTP method to use
    #[structopt(
        short = "m",
        long = "method",
        default_value = "GET",
        value_name = "METHOD",
        parse(try_from_str = "parse_method")
    )]
    method: Method,

    /// URL to request
    #[structopt(name = "URL")]
    url: Url,
}

fn main() {
    let config = Config::from_args();

    tokio::run(run(config));
}

fn run(config: Config) -> impl Future<Item = (), Error = ()> {
    let client = Client::new();

    let url = config.url.into_string();
    let request_count = config.request_count;
    let concurrent_count = config.concurrent_count;
    let method = config.method;

    let requests = (0..request_count)
        .into_iter()
        .map(move |_| make_request(&client, &url, &method));

    let outcomes = stream::iter_ok(requests)
        .buffer_unordered(concurrent_count)
        .collect();

    timed(outcomes)
        .map(move |(outcomes_result, duration)| match outcomes_result {
            Ok(outcomes) => TestOutcome::new(outcomes, duration, concurrent_count),
            _ => unreachable!("The outcomes future cannot fail"),
        })
        .map(|test_outcome| println!("{}", test_outcome))
}

fn make_request(
    client: &Client,
    url: &str,
    method: &Method,
) -> impl Future<Item = RequestOutcome, Error = ()> + Send {
    let request = client.request(method.clone(), url);

    timed(request.send()).map(move |(result, duration)| {
        let result = result.map(|resp| resp.status());
        RequestOutcome::new(result, duration, 0)
    })
}

#[derive(Debug)]
enum FutureState {
    Unpolled,
    Polled(std::time::Instant),
}
struct TimedFuture<F: Future> {
    inner: F,
    state: FutureState,
}

fn timed<F: Future>(future: F) -> TimedFuture<F> {
    TimedFuture {
        inner: future,
        state: FutureState::Unpolled,
    }
}

impl<F> Future for TimedFuture<F>
where
    F: Future,
{
    type Item = (Result<F::Item, F::Error>, Duration);
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

type RequestResult = Result<reqwest::StatusCode, reqwest::Error>;
struct RequestOutcome {
    result: RequestResult,
    duration: Duration,
    // payload_size: usize,
}
impl RequestOutcome {
    fn new(result: RequestResult, duration: Duration, _payload_size: usize) -> RequestOutcome {
        RequestOutcome {
            result,
            duration,
            // payload_size,
        }
    }
}

struct TestOutcome {
    request_outcomes: Vec<RequestOutcome>,
    total_time: Duration,
    concurrent_count: usize,
}

impl TestOutcome {
    fn new(
        request_outcomes: Vec<RequestOutcome>,
        total_time: Duration,
        concurrent_count: usize,
    ) -> TestOutcome {
        TestOutcome {
            request_outcomes,
            total_time,
            concurrent_count,
        }
    }
}

impl std::fmt::Display for TestOutcome {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let total_requests = self.request_outcomes.len();
        let (ok_count, server_err_count, err_count) =
            self.request_outcomes
                .iter()
                .fold((0, 0, 0), |(ok, server_err, err), next| match next.result {
                    Ok(s) if s.is_server_error() => (ok + 1, server_err + 1, err),
                    Ok(_) => (ok + 1, server_err, err),
                    Err(_) => (ok, server_err, err + 1),
                });
        let mut durations: Vec<Duration> = self
            .request_outcomes
            .iter()
            .map(|outcome| outcome.duration)
            .collect();
        let total_time_in_flight: Duration = durations.iter().cloned().sum();
        let avg_time_in_flight = total_time_in_flight / total_requests as u32;

        durations.sort_unstable();

        let fifty_percent = percenteil(&durations, 0.5);
        let sixty_six = percenteil(&durations, 0.66);
        let seventy_five_percent = percenteil(&durations, 0.75);
        let eighty = percenteil(&durations, 0.80);
        let ninety = percenteil(&durations, 0.90);
        let ninety_five = percenteil(&durations, 0.95);
        let ninety_nine = percenteil(&durations, 0.99);
        let longest = durations.last().unwrap();

        writeln!(f, "Total Requests: {:?}", total_requests)?;
        writeln!(f, "Concurrency Count: {}", self.concurrent_count)?;
        writeln!(f, "Total Completed Requests: {:?}", ok_count)?;
        writeln!(f, "Total Errored Requests: {:?}", err_count)?;
        writeln!(f, "Total 5XX Requests: {:?}", server_err_count)?;
        writeln!(f, "");
        writeln!(f, "Total Time Taken: {:?}", self.total_time)?;
        writeln!(
            f,
            "Avg Time Taken: {:?}",
            self.total_time / total_requests as u32
        )?;
        writeln!(f, "Total Time In Flight: {:?}", total_time_in_flight)?;
        writeln!(f, "Avg Time In Flight: {:?}", avg_time_in_flight)?;
        writeln!(f, "");
        writeln!(
            f,
            "Percentage of the requests served within a certain time:"
        );
        writeln!(f, "50%: {:?}", fifty_percent)?;
        writeln!(f, "66%: {:?}", sixty_six)?;
        writeln!(f, "75%: {:?}", seventy_five_percent)?;
        writeln!(f, "80%: {:?}", eighty)?;
        writeln!(f, "90%: {:?}", ninety)?;
        writeln!(f, "95%: {:?}", ninety_five)?;
        writeln!(f, "99%: {:?}", ninety_nine)?;
        writeln!(f, "100%: {:?}", longest)?;

        Ok(())
    }
}

fn percenteil(durations: &Vec<Duration>, percentage: f64) -> Duration {
    let last_index = durations.len() - 1;
    let i = last_index as f64 * percentage;
    let ceil = i.ceil();
    if (ceil as usize) >= last_index {
        return *durations.last().unwrap();
    }

    if i != ceil {
        durations[ceil as usize] + durations[ceil as usize + 1] / 2
    } else {
        durations[ceil as usize]
    }
}

fn parse_method(str: &str) -> Result<Method, String> {
    Method::try_from(str).map_err(|_| format!("unrecognized method: {}", str))
}

fn parse_number_of_requests(n: String) -> Result<(), String> {
    n.parse::<usize>()
        .map_err(|_| format!("'{}' is not a valid positive number", n))
        .and_then(|n| {
            if n != 0 {
                Ok(())
            } else {
                Err(String::from("number of requests must be greater than 0"))
            }
        })
}
