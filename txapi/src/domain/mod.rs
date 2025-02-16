pub mod prelude {
    pub use super::tx::Tx;
}

pub mod tx {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize, Debug, Clone)]
    pub struct Tx {
        pub id: String,
        pub timestamp: String,
        pub cc_number: String,
        pub category: String,
        pub amount_usd_cents: u64,
        pub latitude: f64,
        pub longitude: f64,
        pub country_iso: String,
        pub city: String,
    }
}
