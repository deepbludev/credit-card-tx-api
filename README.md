# Credit Card Transactions Websocket API
Mocked credit card transactions Websocket API for experimental/educational purposes.

## Overview
This project is a simple credit card transactions Websocket API. It is a mock of a real API that would be used to get credit card transactions,
useful for experimental/educational purposes, such as testing and developing ML models for fraud detection.

### Channels
The websocketAPI has two channels:
- `heartbeat`: for checking if the connection is alive
- `transactions`: for getting credit card transactions in realtime

## Local Setup
Run `make run` to start the API locally.
The API will be available at `ws://localhost:9999/v1`.

## API Reference



## Subscribing to Channels

To subscribe to a channel, you need to send a message to the server with the following format:

```json
{
  "method": "subscribe",
  "params": {
    "channel": "transactions"
  }
}
```
### Transactions Response

```json
{
  "channel": "transactions",
  "data": [
    {
      "id": "11df919988c134d97bbff2678eb68e22",
      "timestamp": "2024-01-01T00:00:00Z",
      "cc_number": "4473593503484549",
      "category": "Grocery",
      "amount_usd_cents": 10000,
      "latitude": 37.774929,
      "longitude": -122.419418,
      "country_iso": "US",
      "city": "San Francisco",
    }
  ]
}
```

### Heartbeat

```json
{
  "channel": "heartbeat",
  "data": {
    "status": "ok"
  }
}
```


