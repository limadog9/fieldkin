use fieldkin::algorithms::{
    Coma, Cupid, DistributionBased, JaccardDistanceMatcher, SimilarityFlooding,
};
use fieldkin::{Matcher, Table, valentine_match};
use std::{
    fs::File,
    io::{self, Write},
    path::Path,
};

fn read_table(path: &Path, name: &str) -> Result<Table, fieldkin::Error> {
    let file = File::open(path)?;
    if path
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
    {
        Table::from_json(name, file)
    } else {
        Table::from_csv(name, file)
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "-h" {
        writeln!(
            io::stdout().lock(),
            "Usage: fieldkin <coma|cupid|distribution|jaccard|flooding> <source.csv|json> <target.csv|json> [source_name] [target_name]\nOutputs ranked table-aware matches and component scores as JSON."
        )?;
        return Ok(());
    }
    if !(3..=5).contains(&args.len()) {
        return Err("expected an algorithm and two input files; use --help".into());
    }
    let algorithm = args[0].to_str().ok_or("algorithm must be valid UTF-8")?;
    let matcher: Box<dyn Matcher> = match algorithm {
        "coma" => Box::new(Coma::default()),
        "cupid" => Box::new(Cupid::default()),
        "distribution" => Box::new(DistributionBased::default()),
        "jaccard" => Box::new(JaccardDistanceMatcher::default()),
        "flooding" => Box::new(SimilarityFlooding::default()),
        _ => return Err(format!("unknown algorithm: {algorithm}").into()),
    };
    let source_name = args.get(3).map_or(Ok("aaa"), |name| {
        name.to_str().ok_or("source table name must be valid UTF-8")
    })?;
    let target_name = args.get(4).map_or(Ok("bbb"), |name| {
        name.to_str().ok_or("target table name must be valid UTF-8")
    })?;
    let source = read_table(Path::new(&args[1]), source_name)?;
    let target = read_table(Path::new(&args[2]), target_name)?;
    let matches = valentine_match(&[source, target], matcher.as_ref())?;
    writeln!(
        io::stdout().lock(),
        "{}",
        serde_json::to_string_pretty(&matches)?
    )?;
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        let _ = writeln!(io::stderr().lock(), "fieldkin: {error}");
        std::process::exit(1);
    }
}
