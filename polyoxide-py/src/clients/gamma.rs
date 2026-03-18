use pyo3::prelude::*;
use pyo3::types::PyModuleMethods;
use std::sync::Arc;

use crate::error::gamma_err;
use crate::types::*;

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Markets
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyGammaMarkets,
    sync_name = PyGammaMarketsSync,
    py_async_name = "GammaMarkets",
    py_sync_name = "GammaMarketsSync",
    client_type = polyoxide_gamma::Gamma,
    client_var = client,
    #[pyo3(signature = (id, *, include_tag=None))]
    fn get(id: String, include_tag: Option<bool>) -> PyMarket {
        let mut req = client.markets().get(id);
        if let Some(v) = include_tag {
            req = req.include_tag(v);
        }
        Ok(PyMarket::from(req.send().await.map_err(gamma_err)?))
    },
    #[pyo3(signature = (slug, *, include_tag=None))]
    fn get_by_slug(slug: String, include_tag: Option<bool>) -> PyMarket {
        let mut req = client.markets().get_by_slug(slug);
        if let Some(v) = include_tag {
            req = req.include_tag(v);
        }
        Ok(PyMarket::from(req.send().await.map_err(gamma_err)?))
    },
    #[pyo3(signature = (*, limit=None, offset=None, order=None, ascending=None, id=None, slug=None, clob_token_ids=None, condition_ids=None, liquidity_num_min=None, liquidity_num_max=None, volume_num_min=None, volume_num_max=None, tag_id=None, related_tags=None, closed=None, open=None, archived=None))]
    #[allow(clippy::too_many_arguments)]
    fn list(
        limit: Option<u32>,
        offset: Option<u32>,
        order: Option<String>,
        ascending: Option<bool>,
        id: Option<Vec<i64>>,
        slug: Option<Vec<String>>,
        clob_token_ids: Option<Vec<String>>,
        condition_ids: Option<Vec<String>>,
        liquidity_num_min: Option<f64>,
        liquidity_num_max: Option<f64>,
        volume_num_min: Option<f64>,
        volume_num_max: Option<f64>,
        tag_id: Option<i64>,
        related_tags: Option<bool>,
        closed: Option<bool>,
        open: Option<bool>,
        archived: Option<bool>,
    ) -> Vec<PyMarket> {
        let mut req = client.markets().list();
        if let Some(v) = limit {
            req = req.limit(v);
        }
        if let Some(v) = offset {
            req = req.offset(v);
        }
        if let Some(v) = order {
            req = req.order(v);
        }
        if let Some(v) = ascending {
            req = req.ascending(v);
        }
        if let Some(v) = id {
            req = req.id(v);
        }
        if let Some(v) = slug {
            req = req.slug(v);
        }
        if let Some(v) = clob_token_ids {
            req = req.clob_token_ids(v);
        }
        if let Some(v) = condition_ids {
            req = req.condition_ids(v);
        }
        if let Some(v) = liquidity_num_min {
            req = req.liquidity_num_min(v);
        }
        if let Some(v) = liquidity_num_max {
            req = req.liquidity_num_max(v);
        }
        if let Some(v) = volume_num_min {
            req = req.volume_num_min(v);
        }
        if let Some(v) = volume_num_max {
            req = req.volume_num_max(v);
        }
        if let Some(v) = tag_id {
            req = req.tag_id(v);
        }
        if let Some(v) = related_tags {
            req = req.related_tags(v);
        }
        if let Some(v) = closed {
            req = req.closed(v);
        }
        if let Some(v) = open {
            req = req.open(v);
        }
        if let Some(v) = archived {
            req = req.archived(v);
        }
        let result = req.send().await.map_err(gamma_err)?;
        Ok(result.into_iter().map(PyMarket::from).collect::<Vec<_>>())
    },
    #[pyo3(signature = (id,))]
    fn tags(id: String) -> Vec<PyTag> {
        let result = client.markets().tags(id).send().await.map_err(gamma_err)?;
        Ok(result.into_iter().map(PyTag::from).collect::<Vec<_>>())
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Events
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyGammaEvents,
    sync_name = PyGammaEventsSync,
    py_async_name = "GammaEvents",
    py_sync_name = "GammaEventsSync",
    client_type = polyoxide_gamma::Gamma,
    client_var = client,
    #[pyo3(signature = (*, limit=None, offset=None, order=None, ascending=None, id=None, slug=None, tag_id=None, tag_slug=None, active=None, archived=None, featured=None, closed=None, liquidity_min=None, liquidity_max=None, volume_min=None, volume_max=None))]
    #[allow(clippy::too_many_arguments)]
    fn list(
        limit: Option<u32>,
        offset: Option<u32>,
        order: Option<String>,
        ascending: Option<bool>,
        id: Option<Vec<i64>>,
        slug: Option<Vec<String>>,
        tag_id: Option<i64>,
        tag_slug: Option<String>,
        active: Option<bool>,
        archived: Option<bool>,
        featured: Option<bool>,
        closed: Option<bool>,
        liquidity_min: Option<f64>,
        liquidity_max: Option<f64>,
        volume_min: Option<f64>,
        volume_max: Option<f64>,
    ) -> Vec<PyEvent> {
        let mut req = client.events().list();
        if let Some(v) = limit {
            req = req.limit(v);
        }
        if let Some(v) = offset {
            req = req.offset(v);
        }
        if let Some(v) = order {
            req = req.order(v);
        }
        if let Some(v) = ascending {
            req = req.ascending(v);
        }
        if let Some(v) = id {
            req = req.id(v);
        }
        if let Some(v) = slug {
            req = req.slug(v);
        }
        if let Some(v) = tag_id {
            req = req.tag_id(v);
        }
        if let Some(v) = tag_slug {
            req = req.tag_slug(v);
        }
        if let Some(v) = active {
            req = req.active(v);
        }
        if let Some(v) = archived {
            req = req.archived(v);
        }
        if let Some(v) = featured {
            req = req.featured(v);
        }
        if let Some(v) = closed {
            req = req.closed(v);
        }
        if let Some(v) = liquidity_min {
            req = req.liquidity_min(v);
        }
        if let Some(v) = liquidity_max {
            req = req.liquidity_max(v);
        }
        if let Some(v) = volume_min {
            req = req.volume_min(v);
        }
        if let Some(v) = volume_max {
            req = req.volume_max(v);
        }
        let result = req.send().await.map_err(gamma_err)?;
        Ok(result.into_iter().map(PyEvent::from).collect::<Vec<_>>())
    },
    #[pyo3(signature = (id, *, include_chat=None, include_template=None))]
    fn get(id: String, include_chat: Option<bool>, include_template: Option<bool>) -> PyEvent {
        let mut req = client.events().get(id);
        if let Some(v) = include_chat {
            req = req.include_chat(v);
        }
        if let Some(v) = include_template {
            req = req.include_template(v);
        }
        Ok(PyEvent::from(req.send().await.map_err(gamma_err)?))
    },
    #[pyo3(signature = (slug, *, include_chat=None, include_template=None))]
    fn get_by_slug(
        slug: String,
        include_chat: Option<bool>,
        include_template: Option<bool>,
    ) -> PyEvent {
        let mut req = client.events().get_by_slug(slug);
        if let Some(v) = include_chat {
            req = req.include_chat(v);
        }
        if let Some(v) = include_template {
            req = req.include_template(v);
        }
        Ok(PyEvent::from(req.send().await.map_err(gamma_err)?))
    },
    #[pyo3(signature = (slug,))]
    fn get_related_by_slug(slug: String) -> Vec<PyEvent> {
        let result = client
            .events()
            .get_related_by_slug(slug)
            .send()
            .await
            .map_err(gamma_err)?;
        Ok(result.into_iter().map(PyEvent::from).collect::<Vec<_>>())
    },
    #[pyo3(signature = (id,))]
    fn tags(id: String) -> Vec<PyTag> {
        let result = client.events().tags(id).send().await.map_err(gamma_err)?;
        Ok(result.into_iter().map(PyTag::from).collect::<Vec<_>>())
    },
    #[pyo3(signature = (id,))]
    fn tweet_count(id: String) -> PyCountResponse {
        Ok(PyCountResponse::from(
            client
                .events()
                .tweet_count(id)
                .send()
                .await
                .map_err(gamma_err)?,
        ))
    },
    #[pyo3(signature = (id,))]
    fn comment_count(id: String) -> PyCountResponse {
        Ok(PyCountResponse::from(
            client
                .events()
                .comment_count(id)
                .send()
                .await
                .map_err(gamma_err)?,
        ))
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Series
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyGammaSeries,
    sync_name = PyGammaSeriesSync,
    py_async_name = "GammaSeries",
    py_sync_name = "GammaSeriesSync",
    client_type = polyoxide_gamma::Gamma,
    client_var = client,
    #[pyo3(signature = (*, limit=None, offset=None, ascending=None, closed=None, slug=None, include_chat=None, recurrence=None))]
    fn list(
        limit: Option<u32>,
        offset: Option<u32>,
        ascending: Option<bool>,
        closed: Option<bool>,
        slug: Option<Vec<String>>,
        include_chat: Option<bool>,
        recurrence: Option<String>,
    ) -> Vec<PySeriesData> {
        let mut req = client.series().list();
        if let Some(v) = limit {
            req = req.limit(v);
        }
        if let Some(v) = offset {
            req = req.offset(v);
        }
        if let Some(v) = ascending {
            req = req.ascending(v);
        }
        if let Some(v) = closed {
            req = req.closed(v);
        }
        if let Some(v) = slug {
            req = req.slug(v);
        }
        if let Some(v) = include_chat {
            req = req.include_chat(v);
        }
        if let Some(v) = recurrence {
            req = req.recurrence(v);
        }
        let result = req.send().await.map_err(gamma_err)?;
        Ok(result
            .into_iter()
            .map(PySeriesData::from)
            .collect::<Vec<_>>())
    },
    #[pyo3(signature = (id, *, include_chat=None))]
    fn get(id: String, include_chat: Option<bool>) -> PySeriesData {
        let mut req = client.series().get(id);
        if let Some(v) = include_chat {
            req = req.include_chat(v);
        }
        Ok(PySeriesData::from(req.send().await.map_err(gamma_err)?))
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Tags
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyGammaTags,
    sync_name = PyGammaTagsSync,
    py_async_name = "GammaTags",
    py_sync_name = "GammaTagsSync",
    client_type = polyoxide_gamma::Gamma,
    client_var = client,
    #[pyo3(signature = (*, limit=None, offset=None, order=None, ascending=None, include_template=None, is_carousel=None))]
    fn list(
        limit: Option<u32>,
        offset: Option<u32>,
        order: Option<String>,
        ascending: Option<bool>,
        include_template: Option<bool>,
        is_carousel: Option<bool>,
    ) -> Vec<PyTag> {
        let mut req = client.tags().list();
        if let Some(v) = limit {
            req = req.limit(v);
        }
        if let Some(v) = offset {
            req = req.offset(v);
        }
        if let Some(v) = order {
            req = req.order(v);
        }
        if let Some(v) = ascending {
            req = req.ascending(v);
        }
        if let Some(v) = include_template {
            req = req.include_template(v);
        }
        if let Some(v) = is_carousel {
            req = req.is_carousel(v);
        }
        let result = req.send().await.map_err(gamma_err)?;
        Ok(result.into_iter().map(PyTag::from).collect::<Vec<_>>())
    },
    #[pyo3(signature = (id,))]
    fn get(id: String) -> PyTag {
        Ok(PyTag::from(
            client.tags().get(id).send().await.map_err(gamma_err)?,
        ))
    },
    #[pyo3(signature = (slug,))]
    fn get_by_slug(slug: String) -> PyTag {
        Ok(PyTag::from(
            client
                .tags()
                .get_by_slug(slug)
                .send()
                .await
                .map_err(gamma_err)?,
        ))
    },
    #[pyo3(signature = (id, *, omit_empty=None, status=None))]
    fn get_related(id: String, omit_empty: Option<bool>, status: Option<String>) -> Vec<PyTag> {
        let mut req = client.tags().get_related(id);
        if let Some(v) = omit_empty {
            req = req.omit_empty(v);
        }
        if let Some(v) = status {
            req = req.status(v);
        }
        let result = req.send().await.map_err(gamma_err)?;
        Ok(result.into_iter().map(PyTag::from).collect::<Vec<_>>())
    },
    #[pyo3(signature = (slug, *, omit_empty=None, status=None))]
    fn get_related_by_slug(
        slug: String,
        omit_empty: Option<bool>,
        status: Option<String>,
    ) -> Vec<PyTag> {
        let mut req = client.tags().get_related_by_slug(slug);
        if let Some(v) = omit_empty {
            req = req.omit_empty(v);
        }
        if let Some(v) = status {
            req = req.status(v);
        }
        let result = req.send().await.map_err(gamma_err)?;
        Ok(result.into_iter().map(PyTag::from).collect::<Vec<_>>())
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Comments
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyGammaComments,
    sync_name = PyGammaCommentsSync,
    py_async_name = "GammaComments",
    py_sync_name = "GammaCommentsSync",
    client_type = polyoxide_gamma::Gamma,
    client_var = client,
    #[pyo3(signature = (*, limit=None, offset=None, order=None, ascending=None, parent_entity_type=None, parent_entity_id=None, get_positions=None, holders_only=None))]
    #[allow(clippy::too_many_arguments)]
    fn list(
        limit: Option<u32>,
        offset: Option<u32>,
        order: Option<String>,
        ascending: Option<bool>,
        parent_entity_type: Option<String>,
        parent_entity_id: Option<i64>,
        get_positions: Option<bool>,
        holders_only: Option<bool>,
    ) -> Vec<PyComment> {
        let mut req = client.comments().list();
        if let Some(v) = limit {
            req = req.limit(v);
        }
        if let Some(v) = offset {
            req = req.offset(v);
        }
        if let Some(v) = order {
            req = req.order(v);
        }
        if let Some(v) = ascending {
            req = req.ascending(v);
        }
        if let Some(v) = parent_entity_type {
            req = req.parent_entity_type(v);
        }
        if let Some(v) = parent_entity_id {
            req = req.parent_entity_id(v);
        }
        if let Some(v) = get_positions {
            req = req.get_positions(v);
        }
        if let Some(v) = holders_only {
            req = req.holders_only(v);
        }
        let result = req.send().await.map_err(gamma_err)?;
        Ok(result.into_iter().map(PyComment::from).collect::<Vec<_>>())
    },
    #[pyo3(signature = (id,))]
    fn get(id: String) -> PyComment {
        Ok(PyComment::from(
            client.comments().get(id).send().await.map_err(gamma_err)?,
        ))
    },
    #[pyo3(signature = (address,))]
    fn by_user(address: String) -> Vec<PyComment> {
        let result = client
            .comments()
            .by_user(address)
            .send()
            .await
            .map_err(gamma_err)?;
        Ok(result.into_iter().map(PyComment::from).collect::<Vec<_>>())
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Sports
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyGammaSports,
    sync_name = PyGammaSportsSync,
    py_async_name = "GammaSports",
    py_sync_name = "GammaSportsSync",
    client_type = polyoxide_gamma::Gamma,
    client_var = client,
    #[pyo3(signature = ())]
    fn list() -> Vec<PySportMetadata> {
        let result = client.sports().list().send().await.map_err(gamma_err)?;
        Ok(result
            .into_iter()
            .map(PySportMetadata::from)
            .collect::<Vec<_>>())
    },
    #[pyo3(signature = ())]
    fn market_types() -> Py<PyAny> {
        let result = client
            .sports()
            .market_types()
            .send()
            .await
            .map_err(gamma_err)?;
        pyo3::Python::attach(|py| crate::convert::value_to_pyobject(py, &result))
    },
    #[pyo3(signature = (*, limit=None, offset=None, order=None, ascending=None, league=None, name=None, abbreviation=None))]
    fn list_teams(
        limit: Option<u32>,
        offset: Option<u32>,
        order: Option<String>,
        ascending: Option<bool>,
        league: Option<Vec<String>>,
        name: Option<Vec<String>>,
        abbreviation: Option<Vec<String>>,
    ) -> Vec<PyTeam> {
        let mut req = client.sports().list_teams();
        if let Some(v) = limit {
            req = req.limit(v);
        }
        if let Some(v) = offset {
            req = req.offset(v);
        }
        if let Some(v) = order {
            req = req.order(v);
        }
        if let Some(v) = ascending {
            req = req.ascending(v);
        }
        if let Some(v) = league {
            req = req.league(v);
        }
        if let Some(v) = name {
            req = req.name(v);
        }
        if let Some(v) = abbreviation {
            req = req.abbreviation(v);
        }
        let result = req.send().await.map_err(gamma_err)?;
        Ok(result.into_iter().map(PyTeam::from).collect::<Vec<_>>())
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Search
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyGammaSearch,
    sync_name = PyGammaSearchSync,
    py_async_name = "GammaSearch",
    py_sync_name = "GammaSearchSync",
    client_type = polyoxide_gamma::Gamma,
    client_var = client,
    #[pyo3(signature = (query, *, limit_per_type=None, page=None, cache=None, events_status=None, events_tag=None, keep_closed_markets=None, sort=None, search_tags=None, search_profiles=None))]
    #[allow(clippy::too_many_arguments)]
    fn public_search(
        query: String,
        limit_per_type: Option<u32>,
        page: Option<u32>,
        cache: Option<bool>,
        events_status: Option<String>,
        events_tag: Option<Vec<i64>>,
        keep_closed_markets: Option<bool>,
        sort: Option<String>,
        search_tags: Option<bool>,
        search_profiles: Option<bool>,
    ) -> PySearchResponse {
        let mut req = client.search().public_search(query);
        if let Some(v) = limit_per_type {
            req = req.limit_per_type(v);
        }
        if let Some(v) = page {
            req = req.page(v);
        }
        if let Some(v) = cache {
            req = req.cache(v);
        }
        if let Some(v) = events_status {
            req = req.events_status(v);
        }
        if let Some(v) = events_tag {
            req = req.events_tag(v);
        }
        if let Some(v) = keep_closed_markets {
            req = req.keep_closed_markets(v);
        }
        if let Some(v) = sort {
            req = req.sort(v);
        }
        if let Some(v) = search_tags {
            req = req.search_tags(v);
        }
        if let Some(v) = search_profiles {
            req = req.search_profiles(v);
        }
        Ok(PySearchResponse::from(req.send().await.map_err(gamma_err)?))
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: User
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyGammaUser,
    sync_name = PyGammaUserSync,
    py_async_name = "GammaUser",
    py_sync_name = "GammaUserSync",
    client_type = polyoxide_gamma::Gamma,
    client_var = client,
    #[pyo3(signature = (address,))]
    fn get(address: String) -> PyUserResponse {
        Ok(PyUserResponse::from(
            client.user().get(address).send().await.map_err(gamma_err)?,
        ))
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Namespace: Health
// ═══════════════════════════════════════════════════════════════════════════════

client_ns!(
    async_name = PyGammaHealth,
    sync_name = PyGammaHealthSync,
    py_async_name = "GammaHealth",
    py_sync_name = "GammaHealthSync",
    client_type = polyoxide_gamma::Gamma,
    client_var = client,
    #[pyo3(signature = ())]
    fn ping() -> f64 {
        let duration = client.health().ping().await.map_err(gamma_err)?;
        Ok(duration.as_secs_f64())
    }
);

// ═══════════════════════════════════════════════════════════════════════════════
// Async Client
// ═══════════════════════════════════════════════════════════════════════════════

#[pyclass(name = "Gamma", skip_from_py_object)]
pub struct PyGamma {
    client: Arc<polyoxide_gamma::Gamma>,
}

#[pymethods]
impl PyGamma {
    #[new]
    #[pyo3(signature = (*, base_url=None, timeout_ms=None, pool_size=None))]
    fn new(
        base_url: Option<String>,
        timeout_ms: Option<u64>,
        pool_size: Option<usize>,
    ) -> PyResult<Self> {
        let mut builder = polyoxide_gamma::Gamma::builder();
        if let Some(v) = base_url {
            builder = builder.base_url(v);
        }
        if let Some(v) = timeout_ms {
            builder = builder.timeout_ms(v);
        }
        if let Some(v) = pool_size {
            builder = builder.pool_size(v);
        }
        let client = builder.build().map_err(gamma_err)?;
        Ok(Self {
            client: Arc::new(client),
        })
    }

    fn markets(&self) -> PyGammaMarkets {
        PyGammaMarkets {
            client: self.client.clone(),
        }
    }

    fn events(&self) -> PyGammaEvents {
        PyGammaEvents {
            client: self.client.clone(),
        }
    }

    fn series(&self) -> PyGammaSeries {
        PyGammaSeries {
            client: self.client.clone(),
        }
    }

    fn tags(&self) -> PyGammaTags {
        PyGammaTags {
            client: self.client.clone(),
        }
    }

    fn comments(&self) -> PyGammaComments {
        PyGammaComments {
            client: self.client.clone(),
        }
    }

    fn sports(&self) -> PyGammaSports {
        PyGammaSports {
            client: self.client.clone(),
        }
    }

    fn search(&self) -> PyGammaSearch {
        PyGammaSearch {
            client: self.client.clone(),
        }
    }

    fn user(&self) -> PyGammaUser {
        PyGammaUser {
            client: self.client.clone(),
        }
    }

    fn health(&self) -> PyGammaHealth {
        PyGammaHealth {
            client: self.client.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Sync Client
// ═══════════════════════════════════════════════════════════════════════════════

#[pyclass(name = "GammaSync", skip_from_py_object)]
pub struct PyGammaSync {
    client: Arc<polyoxide_gamma::Gamma>,
}

#[pymethods]
impl PyGammaSync {
    #[new]
    #[pyo3(signature = (*, base_url=None, timeout_ms=None, pool_size=None))]
    fn new(
        base_url: Option<String>,
        timeout_ms: Option<u64>,
        pool_size: Option<usize>,
    ) -> PyResult<Self> {
        let mut builder = polyoxide_gamma::Gamma::builder();
        if let Some(v) = base_url {
            builder = builder.base_url(v);
        }
        if let Some(v) = timeout_ms {
            builder = builder.timeout_ms(v);
        }
        if let Some(v) = pool_size {
            builder = builder.pool_size(v);
        }
        let client = builder.build().map_err(gamma_err)?;
        Ok(Self {
            client: Arc::new(client),
        })
    }

    fn markets(&self) -> PyGammaMarketsSync {
        PyGammaMarketsSync {
            client: self.client.clone(),
        }
    }

    fn events(&self) -> PyGammaEventsSync {
        PyGammaEventsSync {
            client: self.client.clone(),
        }
    }

    fn series(&self) -> PyGammaSeriesSync {
        PyGammaSeriesSync {
            client: self.client.clone(),
        }
    }

    fn tags(&self) -> PyGammaTagsSync {
        PyGammaTagsSync {
            client: self.client.clone(),
        }
    }

    fn comments(&self) -> PyGammaCommentsSync {
        PyGammaCommentsSync {
            client: self.client.clone(),
        }
    }

    fn sports(&self) -> PyGammaSportsSync {
        PyGammaSportsSync {
            client: self.client.clone(),
        }
    }

    fn search(&self) -> PyGammaSearchSync {
        PyGammaSearchSync {
            client: self.client.clone(),
        }
    }

    fn user(&self) -> PyGammaUserSync {
        PyGammaUserSync {
            client: self.client.clone(),
        }
    }

    fn health(&self) -> PyGammaHealthSync {
        PyGammaHealthSync {
            client: self.client.clone(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════════
// Registration
// ═══════════════════════════════════════════════════════════════════════════════

pub fn register(m: &pyo3::Bound<'_, pyo3::types::PyModule>) -> pyo3::PyResult<()> {
    m.add_class::<PyGamma>()?;
    m.add_class::<PyGammaSync>()?;
    m.add_class::<PyGammaMarkets>()?;
    m.add_class::<PyGammaEvents>()?;
    m.add_class::<PyGammaSeries>()?;
    m.add_class::<PyGammaTags>()?;
    m.add_class::<PyGammaComments>()?;
    m.add_class::<PyGammaSports>()?;
    m.add_class::<PyGammaSearch>()?;
    m.add_class::<PyGammaUser>()?;
    m.add_class::<PyGammaHealth>()?;
    m.add_class::<PyGammaMarketsSync>()?;
    m.add_class::<PyGammaEventsSync>()?;
    m.add_class::<PyGammaSeriesSync>()?;
    m.add_class::<PyGammaTagsSync>()?;
    m.add_class::<PyGammaCommentsSync>()?;
    m.add_class::<PyGammaSportsSync>()?;
    m.add_class::<PyGammaSearchSync>()?;
    m.add_class::<PyGammaUserSync>()?;
    m.add_class::<PyGammaHealthSync>()?;
    Ok(())
}
