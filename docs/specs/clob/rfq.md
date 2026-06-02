# CLOB RFQ (Request for Quote)

Base URL: `https://clob.polymarket.com`

> Note: RFQ endpoints are not in the upstream OpenAPI spec, and as of this sync every path below returns HTTP 404 at `clob.polymarket.com` (verified against the live API). They appear to have been removed or relocated; this documentation is based on other (likely stale) sources. Do not rely on these paths without upstream confirmation.

## Create RFQ Request

`POST /rfq/request`

**Auth:** L2

## Cancel RFQ Request

`DELETE /rfq/request`

**Auth:** L2

## Create RFQ Quote

`POST /rfq/quote`

**Auth:** L2

## Cancel RFQ Quote

`DELETE /rfq/quote`

**Auth:** L2

## List Quotes

`GET /rfq/quotes`

**Auth:** L2

## List Requests

`GET /rfq/requests`

**Auth:** L2

## Get RFQ Prices

`GET /rfq/prices`

**Auth:** L2
