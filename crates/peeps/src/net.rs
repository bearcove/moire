#[cfg(feature = "diagnostics")]
use compact_str::CompactString;
#[cfg(feature = "diagnostics")]
use peeps_types::{EntityBody, NetEntity};
#[cfg(feature = "diagnostics")]
use std::future::Future;
use std::future::IntoFuture;

#[cfg(feature = "diagnostics")]
fn net_wait<F>(
    future: F,
    endpoint: &str,
    transport: &str,
    op: &'static str,
    body: EntityBody,
) -> impl Future<Output = F::Output>
where
    F: IntoFuture,
{
    let endpoint = CompactString::from(endpoint);
    let transport = CompactString::from(transport);
    let op_handle = crate::EntityHandle::new(format!("net.{op}.{transport}.{endpoint}"), body);
    let wait_name = format!("net.{op}.wait");
    async move { crate::instrument_future_on(wait_name, &op_handle, future.into_future()).await }
}

#[cfg(feature = "diagnostics")]
#[inline]
pub fn connect<F: IntoFuture>(
    future: F,
    endpoint: &str,
    transport: &str,
) -> impl Future<Output = F::Output> {
    net_wait(
        future,
        endpoint,
        transport,
        "connect",
        EntityBody::NetConnect(NetEntity {
            addr: endpoint.into(),
        }),
    )
}

#[cfg(not(feature = "diagnostics"))]
#[inline]
pub fn connect<F: IntoFuture>(future: F, _endpoint: &str, _transport: &str) -> F::IntoFuture {
    future.into_future()
}

#[cfg(feature = "diagnostics")]
#[inline]
pub fn accept<F: IntoFuture>(
    future: F,
    endpoint: &str,
    transport: &str,
) -> impl Future<Output = F::Output> {
    net_wait(
        future,
        endpoint,
        transport,
        "accept",
        EntityBody::NetAccept(NetEntity {
            addr: endpoint.into(),
        }),
    )
}

#[cfg(not(feature = "diagnostics"))]
#[inline]
pub fn accept<F: IntoFuture>(future: F, _endpoint: &str, _transport: &str) -> F::IntoFuture {
    future.into_future()
}

#[cfg(feature = "diagnostics")]
#[inline]
pub fn readable<F: IntoFuture>(
    future: F,
    endpoint: &str,
    transport: &str,
) -> impl Future<Output = F::Output> {
    net_wait(
        future,
        endpoint,
        transport,
        "readable",
        EntityBody::NetRead(NetEntity {
            addr: endpoint.into(),
        }),
    )
}

#[cfg(not(feature = "diagnostics"))]
#[inline]
pub fn readable<F: IntoFuture>(future: F, _endpoint: &str, _transport: &str) -> F::IntoFuture {
    future.into_future()
}

#[cfg(feature = "diagnostics")]
#[inline]
pub fn writable<F: IntoFuture>(
    future: F,
    endpoint: &str,
    transport: &str,
) -> impl Future<Output = F::Output> {
    net_wait(
        future,
        endpoint,
        transport,
        "writable",
        EntityBody::NetWrite(NetEntity {
            addr: endpoint.into(),
        }),
    )
}

#[cfg(not(feature = "diagnostics"))]
#[inline]
pub fn writable<F: IntoFuture>(future: F, _endpoint: &str, _transport: &str) -> F::IntoFuture {
    future.into_future()
}
