use rand::RngExt;
use rand::distr::Alphanumeric;

pub(crate) fn random_alpha_num() -> String {
    rand::rng()
        .sample_iter(Alphanumeric)
        .take(8)
        .map(char::from)
        .collect()
}
