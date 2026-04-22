# Relay Transactions

Base URL: `https://relayer-v2.polymarket.com`

## Submit Transaction

`POST /submit`

**Auth:** Builder API Key or Relayer API Key

**Request:**

```json
{
  "from": "string — signer address",
  "to": "string — target contract",
  "proxyWallet": "string — user's proxy wallet",
  "data": "string — encoded calldata",
  "nonce": "string",
  "signature": "string",
  "signatureParams": {
    "gasPrice": "string",
    "operation": 0,
    "safeTxnGas": "string",
    "baseGas": "string",
    "gasToken": "string",
    "refundReceiver": "string"
  },
  "type": "SAFE | PROXY"
}
```

**Response:**

```json
{
  "transactionID": "string",
  "state": "STATE_NEW"
}
```

Returns immediately with the `transactionID` and a `state` of `STATE_NEW`. The onchain `transactionHash` is **not** included in this response — poll `GET /transaction` with the returned `transactionID` to retrieve the `transactionHash` once the transaction has been broadcast.

## Get Transaction

`GET /transaction`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| id | query | string | yes | Transaction ID |

Poll this endpoint with the `transactionID` returned from `POST /submit` to retrieve the onchain `transactionHash` once the transaction has been broadcast.

**Response:** Array of `RelayerTransaction`

```json
[{
  "transactionID": "string",
  "transactionHash": "string",
  "from": "string",
  "to": "string",
  "proxyAddress": "string",
  "data": "string",
  "nonce": "string",
  "value": "string",
  "signature": "string",
  "state": "STATE_NEW | STATE_EXECUTED | STATE_MINED | STATE_CONFIRMED | STATE_INVALID | STATE_FAILED",
  "type": "SAFE | PROXY",
  "owner": "string",
  "metadata": "string",
  "createdAt": "string",
  "updatedAt": "string"
}]
```

## Get Recent Transactions

`GET /transactions`

**Auth:** Builder API Key or Relayer API Key

**Response:** Array of `RelayerTransaction`

## Get Nonce

`GET /nonce`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| address | query | string | yes | Wallet address |
| type | query | string | yes | PROXY or SAFE |

**Response:**

```json
{"nonce": "string"}
```

## Get Relay Payload

`GET /relay-payload`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| address | query | string | yes | Wallet address |
| type | query | string | yes | PROXY or SAFE |

**Response:**

```json
{"address": "string", "nonce": "string"}
```

## Check Safe Deployment

`GET /deployed`

**Auth:** None

| Name | In | Type | Required | Description |
|------|-----|------|----------|-------------|
| address | query | string | yes | Safe address |

**Response:**

```json
{"deployed": true}
```

## Get Relayer API Keys

`GET /relayer/api/keys`

**Auth:** Relayer API Key

**Response:** Array of `RelayerApiKey`

```json
[{"apiKey": "string", "address": "string", "createdAt": "string", "updatedAt": "string"}]
```

## Verification

```bash
# Check if an address has a deployed Safe
curl -s 'https://relayer-v2.polymarket.com/deployed?address=0xFeA4cB3dD4ca7CefD3368653B7D6FF9BcDFca604' | jq .

# Get nonce
curl -s 'https://relayer-v2.polymarket.com/nonce?address=0xFeA4cB3dD4ca7CefD3368653B7D6FF9BcDFca604&type=SAFE' | jq .
```
