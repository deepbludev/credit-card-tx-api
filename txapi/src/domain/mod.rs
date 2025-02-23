pub mod prelude {
    pub use super::{heartbeat::Heartbeat, transactions::Transaction};
}

pub mod transactions {
    use serde::{Deserialize, Serialize};

    /// Domain model for a Credit Card Transaction.
    ///
    #[derive(Deserialize, Serialize, Debug, Clone)]
    pub struct Transaction {
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

    impl Transaction {
        /// Creates a simple mock transaction with random values.
        ///
        /// TODO: improve this mock implementation
        ///
        pub fn simple_mock() -> Self {
            Self {
                id: uuid::Uuid::new_v4().to_string().replace("-", ""),
                timestamp: chrono::Utc::now().to_rfc3339(),
                cc_number: "4473593503484549".to_string(),
                category: "Grocery".to_string(),
                amount_usd_cents: rand::random::<u64>() % 10000,
                latitude: 37.774929,
                longitude: -122.419418,
                country_iso: "US".to_string(),
                city: "San Francisco".to_string(),
            }
        }
    }
}

pub mod heartbeat {
    use serde::{Deserialize, Serialize};

    #[derive(Deserialize, Serialize, Debug, Clone)]
    pub struct Heartbeat {
        pub status: String,
    }
}
