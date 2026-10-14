use fieldkin::algorithms::{Coma, ComaConfig, JaccardDistanceMatcher};
use fieldkin::metrics::GroundTruth;
use fieldkin::{Table, WorkLimits, valentine_match};

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
    let tables = [customers, orders];
    let matches = valentine_match(&tables, &matcher)?;
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

    // Limits are opt-in for each operation; the COMA and metric calls above
    // remain unbounded. These two-column tables have four column-pair slots
    // and 4 * (2 * 2) = 16 distinct-value comparisons before scoring shortcuts.
    let limits = WorkLimits {
        max_assignment_dimension: 2,
        max_matrix_cells: 4,
        max_clustering_variables: 0,
        max_clustering_constraints: 0,
        max_jaccard_comparisons: 16,
    };
    let value_matches = JaccardDistanceMatcher::default()
        .get_matches_with_limits(&tables[0], &tables[1], &limits)?;
    let selected = value_matches.one_to_one_hungarian_with_limits(Some(0.5), &limits)?;
    assert_eq!(selected.len(), 2);
    println!("Bounded value matches: {}", selected.len());
    // Count limits are not deadlines or process-memory limits. Use external
    // process containment when hard limits are required.
    Ok(())
}
