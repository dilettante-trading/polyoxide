use pyo3::types::PyModuleMethods;

py_type!(PyClobMarket, "ClobMarket", polyoxide_clob::api::markets::Market,
    condition_id, question_id, tokens, rewards,
    minimum_order_size, minimum_tick_size,
    description, category, end_date_iso, question,
    active, closed, archived, neg_risk,
    neg_risk_market_id, enable_order_book,
);

py_type!(PyClobMarketToken, "ClobMarketToken", polyoxide_clob::api::markets::MarketToken,
    token_id, outcome, price, winner,
);

py_type!(PyListMarketsResponse, "ListMarketsResponse", polyoxide_clob::api::markets::ListMarketsResponse,
    data, next_cursor,
);

py_type!(PyOrderBook, "OrderBook", polyoxide_clob::api::markets::OrderBook,
    market, asset_id, bids, asks, timestamp, hash,
    min_order_size, tick_size, neg_risk, last_trade_price,
);

py_type!(PyOrderLevel, "OrderLevel", polyoxide_clob::api::markets::OrderLevel,
    price, size,
);

py_type!(PyPriceResponse, "PriceResponse", polyoxide_clob::api::markets::PriceResponse,
    price,
);

py_type!(PyMidpointResponse, "MidpointResponse", polyoxide_clob::api::markets::MidpointResponse,
    mid,
);

py_type!(PyPriceHistoryPoint, "PriceHistoryPoint", polyoxide_clob::api::markets::PriceHistoryPoint,
    timestamp => "t",
    price => "p",
);

py_type!(PyPricesHistoryResponse, "PricesHistoryResponse", polyoxide_clob::api::markets::PricesHistoryResponse,
    history,
);

py_type!(PyNegRiskResponse, "NegRiskResponse", polyoxide_clob::api::markets::NegRiskResponse,
    neg_risk,
);

py_type!(PyFeeRateResponse, "FeeRateResponse", polyoxide_clob::api::markets::FeeRateResponse,
    base_fee,
);

py_type!(PyTickSizeResponse, "TickSizeResponse", polyoxide_clob::api::markets::TickSizeResponse,
    minimum_tick_size,
);

py_type!(PySpreadResponse, "SpreadResponse", polyoxide_clob::api::markets::SpreadResponse,
    token_id, spread, bid, ask,
);

py_type!(PyLastTradePriceResponse, "LastTradePriceResponse", polyoxide_clob::api::markets::LastTradePriceResponse,
    token_id, last_trade_price, timestamp,
);

py_type!(PyLiveActivityEvent, "LiveActivityEvent", polyoxide_clob::api::markets::LiveActivityEvent,
    condition_id,
);

py_type!(PyCalculatePriceResponse, "CalculatePriceResponse", polyoxide_clob::api::markets::CalculatePriceResponse,
    price,
);

py_type!(PyServerTimeResponse, "ServerTimeResponse", polyoxide_clob::api::health::ServerTimeResponse,
    time,
);

pub fn register(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    m.add_class::<PyClobMarket>()?;
    m.add_class::<PyClobMarketToken>()?;
    m.add_class::<PyListMarketsResponse>()?;
    m.add_class::<PyOrderBook>()?;
    m.add_class::<PyOrderLevel>()?;
    m.add_class::<PyPriceResponse>()?;
    m.add_class::<PyMidpointResponse>()?;
    m.add_class::<PyPriceHistoryPoint>()?;
    m.add_class::<PyPricesHistoryResponse>()?;
    m.add_class::<PyNegRiskResponse>()?;
    m.add_class::<PyFeeRateResponse>()?;
    m.add_class::<PyTickSizeResponse>()?;
    m.add_class::<PySpreadResponse>()?;
    m.add_class::<PyLastTradePriceResponse>()?;
    m.add_class::<PyLiveActivityEvent>()?;
    m.add_class::<PyCalculatePriceResponse>()?;
    m.add_class::<PyServerTimeResponse>()?;
    Ok(())
}
