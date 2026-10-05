//! Native, unpublished tooling. No Python runtime or subprocesses are used.

mod common;
mod imports;
mod performance;
mod qualification;
mod quality;
mod verified;

fn main() {
    let mut args: Vec<_> = std::env::args().skip(1).collect();
    let result = if args.is_empty() {
        Err("choose a command: import-northix, import-t2d".into())
    } else {
        let command = args.remove(0);
        match command.as_str() {
            "import-northix" => imports::northix(args),
            "import-t2d" => imports::t2d(args),
            "verify-build" => verified::build(args),
            "verify-run" => verified::run(args, false),
            "record-northix" => verified::run(args, true),
            "record-context" => verified::record_context(args),
            "record-quality-development" => verified::record_quality_development(args),
            "qualify-build" => qualification::build(args),
            "qualify-run" => qualification::run(args),
            "quality-freeze" => quality::freeze(args),
            "quality-qualify" => quality::qualify(args),
            "release-acceptance" => quality::accept(args),
            "development-acceptance" => quality::accept_development(args),
            "dependency-review" => qualification::dependencies(args),
            command if command.starts_with("perf-") => performance::dispatch(command, args),
            _ => Err(format!("unknown tooling command: {command}")),
        }
    };
    if let Err(error) = result {
        eprintln!("fieldkin-tools: {error}");
        std::process::exit(1);
    }
}
