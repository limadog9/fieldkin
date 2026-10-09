use fieldkin::algorithms::{
    Coma, Cupid, DistributionBased, JaccardDistanceMatcher, SimilarityFlooding,
};
use fieldkin::{Matcher, Table, valentine_match};
use std::{fs::File, path::Path};

fn read_table(path: &str, name: &str) -> Result<Table, fieldkin::Error> {
    let file = File::open(path)?;
    if Path::new(path)
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("json"))
    {
        Table::from_json(name, file)
    } else {
        Table::from_csv(name, file)
    }
}
fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || matches!(args[0].as_str(), "--help" | "-h") {
        println!(
            "Usage: fieldkin <coma|cupid|distribution|jaccard|flooding> <source.csv|json> <target.csv|json> [source_name] [target_name]\nOutputs ranked table-aware matches and component scores as JSON."
        );
        return Ok(());
    }
    if !(3..=5).contains(&args.len()) {
        return Err("expected an algorithm and two input files; use --help".into());
    }
    let matcher: Box<dyn Matcher> = match args[0].as_str() {
        "coma" => Box::new(Coma::default()),
        "cupid" => Box::new(Cupid::default()),
        "distribution" => Box::new(DistributionBased::default()),
        "jaccard" => Box::new(JaccardDistanceMatcher::default()),
        "flooding" => Box::new(SimilarityFlooding::default()),
        _ => return Err(format!("unknown algorithm: {}", args[0]).into()),
    };
    let source = read_table(&args[1], args.get(3).map_or("aaa", String::as_str))?;
    let target = read_table(&args[2], args.get(4).map_or("bbb", String::as_str))?;
    let matches = valentine_match(&[source, target], matcher.as_ref())?;
    println!("{}", serde_json::to_string_pretty(&matches)?);
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("fieldkin: {error}");
        std::process::exit(1);
    }
}
