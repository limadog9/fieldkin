pub(crate) fn canonical_token(token: &str) -> &str {
    match token {
        "supplier" => "vendor",
        "item" => "product",
        "qty" => "quantity",
        "ref" => "reference",
        "txn" => "transaction",
        "key" => "id",
        _ => token,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_synonyms_share_a_canonical_word() {
        assert_eq!(canonical_token("supplier"), "vendor");
        assert_eq!(canonical_token("item"), "product");
        assert_eq!(canonical_token("qty"), "quantity");
    }

    #[test]
    fn unknown_words_are_unchanged() {
        assert_eq!(canonical_token("customer"), "customer");
    }
}
