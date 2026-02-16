use peeps_types::MetaBuilder;

use super::RpcEvent;

#[inline(always)]
pub fn record_request(_event: RpcEvent<'_>) {}

#[inline(always)]
pub fn record_response(_event: RpcEvent<'_>) {}

#[inline(always)]
pub fn record_request_with_meta<const N: usize>(
    _entity_id: &str,
    _name: &str,
    _meta: MetaBuilder<'_, N>,
    _parent_entity_id: Option<&str>,
) {
}

#[inline(always)]
pub fn record_response_with_meta<const N: usize>(
    _entity_id: &str,
    _name: &str,
    _meta: MetaBuilder<'_, N>,
    _parent_entity_id: Option<&str>,
) {
}
