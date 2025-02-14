use serde::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "method")]
pub enum Request {
    #[serde(rename = "subscribe")]
    Subscribe { params: SubscribeParams },
    #[serde(rename = "unsubscribe")]
    Unsubscribe { params: SubscribeParams },
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SubscribeReq {
    pub method: String,
    pub params: SubscribeParams,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct SubscribeParams {
    pub channel: String,
}

#[derive(Deserialize, Serialize, Debug)]
#[serde(tag = "channel")]
pub enum ChannelMsg {
    #[serde(rename = "transactions")]
    Transactions { data: Vec<Transaction> },

    #[serde(rename = "heartbeat")]
    Heartbeat { data: Heartbeat },
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Transaction {
    pub id: String,
    pub timestamp: String,
    #[serde(rename = "cc_number")]
    pub cc_number: String,
    pub category: String,
    #[serde(rename = "amount_usd_cents")]
    pub amount_usd_cents: u64,
    pub latitude: f64,
    pub longitude: f64,
    #[serde(rename = "country_iso")]
    pub country_iso: String,
    pub city: String,
}

#[derive(Deserialize, Serialize, Debug)]
pub struct Heartbeat {
    pub status: String,
}
