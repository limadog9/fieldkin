//! Development-only, deterministic evaluation of the public Fieldkin API.

mod context_policy;
mod context_qualification;
mod corpus;
mod corrective;
mod corrective_corpus;
mod external;
mod metrics;
mod model;
mod northix;
mod precision;
mod readiness;
mod release;
mod runner;
mod snapshot;
mod stage3;

fn main() {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let result = if args
        .first()
        .is_some_and(|arg| arg == "--context-qualification")
    {
        args.remove(0);
        context_qualification::run(args)
    } else if args.first().is_some_and(|arg| arg == "--readiness") {
        args.remove(0);
        readiness::run(args)
    } else if args.first().is_some_and(|arg| arg == "--precision") {
        args.remove(0);
        precision::run(args)
    } else if args.first().is_some_and(|arg| arg == "--northix") {
        args.remove(0);
        northix::run(args)
    } else if args.first().is_some_and(|arg| arg == "--external") {
        args.remove(0);
        external::run(args)
    } else if args.first().is_some_and(|arg| arg == "--corrective") {
        args.remove(0);
        corrective::run(args)
    } else if args.first().is_some_and(|arg| arg == "--release") {
        args.remove(0);
        release::run(args)
    } else if args.first().is_some_and(|arg| arg == "--stage3") {
        args.remove(0);
        stage3::run(args)
    } else {
        runner::run(args)
    };
    if let Err(error) = result {
        eprintln!("fieldkin-eval: {error}");
        std::process::exit(1);
    }
}
