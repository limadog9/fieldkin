use fieldkin::algorithms::{Coma, ComaConfig};
use fieldkin::metrics::GroundTruth;
use fieldkin::{Table, valentine_match};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let customers = Table::from_csv(
        "customers",
        "customer_id,email\n1,alice@example.org\n2,bob@example.org\n".as_bytes(),
    )?;
    let orders = Table::from_csv(
        "orders",
        "customerId,email_address\n1,alice@example.org\n2,bob@example.org\n".as_bytes(),
    )?;
    let matcher = Coma::new(ComaConfig {
        use_instances: true,
        ..Default::default()
    })?;
    let matches = valentine_match(&[customers, orders], &matcher)?;
    for (pair, score) in &matches {
        println!(
            "{}.{} -> {}.{}: {:.3}",
            pair.source_table, pair.source_column, pair.target_table, pair.target_column, score
        );
        println!("  {:?}", matches.get_details(pair));
    }
    let truth = GroundTruth::Names(vec![
        ("customer_id".into(), "customerId".into()),
        ("email".into(), "email_address".into()),
    ]);
    println!("{:?}", matches.get_metrics(&truth)?);
    Ok(())
}
