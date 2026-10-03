//! Development-only, deterministic evaluation of the public Fieldkin API.

mod corpus;
mod metrics;
mod model;
mod runner;

fn main() {
    if let Err(error) = runner::run(std::env::args().skip(1).collect()) {
        eprintln!("fieldkin-eval: {error}");
        std::process::exit(1);
    }
}
