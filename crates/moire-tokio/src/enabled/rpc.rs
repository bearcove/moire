use moire_types::{EdgeKind, EntityBody, EntityId, RequestEntity, ResponseEntity, ResponseStatus};

use super::SourceId;
use moire_runtime::{EntityHandle, EntityRef};

#[derive(Clone)]
pub struct RpcRequestHandle {
    handle: EntityHandle<moire_types::Request>,
}

impl RpcRequestHandle {
    pub fn id(&self) -> &EntityId {
        self.handle.id()
    }

    pub fn id_for_wire(&self) -> String {
        String::from(self.handle.id().as_str())
    }

    pub fn entity_ref(&self) -> EntityRef {
        self.handle.entity_ref()
    }

    #[doc(hidden)]
    pub fn handle(&self) -> &EntityHandle<moire_types::Request> {
        &self.handle
    }
}

#[doc(hidden)]
pub fn rpc_request_with_source(
    method: impl Into<String>,
    args_json: impl Into<String>,
    source: SourceId,
) -> RpcRequestHandle {
    let method = method.into();
    let (service_name, method_name) = split_method_parts(method.as_str());
    let service_name = String::from(service_name);
    let method_name = String::from(method_name);
    rpc_request_with_body(
        method,
        RequestEntity {
            service_name,
            method_name,
            args_json: moire_types::Json::new(args_json),
        },
        source,
    )
}

#[doc(hidden)]
pub fn rpc_request(
    method: impl Into<String>,
    args_json: impl Into<String>,
    source: SourceId,
) -> RpcRequestHandle {
    rpc_request_with_source(method, args_json, source)
}

#[doc(hidden)]
pub fn rpc_request_with_body(
    name: impl Into<String>,
    body: RequestEntity,
    source: SourceId,
) -> RpcRequestHandle {
    let name = name.into();
    let body = EntityBody::Request(body);
    RpcRequestHandle {
        handle: EntityHandle::new(name, body, source).into_typed::<moire_types::Request>(),
    }
}

#[doc(hidden)]
pub fn rpc_response_with_source(
    method: impl Into<String>,
    source: SourceId,
) -> EntityHandle<moire_types::Response> {
    let method = method.into();
    let (service_name, method_name) = split_method_parts(method.as_str());
    let service_name = String::from(service_name);
    let method_name = String::from(method_name);
    rpc_response_with_body(
        method,
        ResponseEntity {
            service_name,
            method_name,
            status: ResponseStatus::Pending,
        },
        source,
    )
}

#[doc(hidden)]
pub fn rpc_response(
    method: impl Into<String>,
    source: SourceId,
) -> EntityHandle<moire_types::Response> {
    rpc_response_with_source(method, source)
}

#[doc(hidden)]
pub fn rpc_response_with_body(
    name: impl Into<String>,
    body: ResponseEntity,
    source: SourceId,
) -> EntityHandle<moire_types::Response> {
    let name = name.into();
    let body = EntityBody::Response(body);
    EntityHandle::new(name, body, source).into_typed::<moire_types::Response>()
}

#[doc(hidden)]
pub fn rpc_response_for_with_source(
    method: impl Into<String>,
    request: &EntityRef,
    source: SourceId,
) -> EntityHandle<moire_types::Response> {
    let method = method.into();
    let (service_name, method_name) = split_method_parts(method.as_str());
    let service_name = String::from(service_name);
    let method_name = String::from(method_name);
    rpc_response_for_with_body(
        method,
        request,
        ResponseEntity {
            service_name,
            method_name,
            status: ResponseStatus::Pending,
        },
        source,
    )
}

#[doc(hidden)]
pub fn rpc_response_for(
    method: impl Into<String>,
    request: &EntityRef,
    source: SourceId,
) -> EntityHandle<moire_types::Response> {
    rpc_response_for_with_source(method, request, source)
}

#[doc(hidden)]
pub fn rpc_response_for_with_body(
    name: impl Into<String>,
    request: &EntityRef,
    body: ResponseEntity,
    source: SourceId,
) -> EntityHandle<moire_types::Response> {
    let name = name.into();
    let body = EntityBody::Response(body);
    let response = EntityHandle::new(name, body, source).into_typed::<moire_types::Response>();
    response.link_to_with_source(request, EdgeKind::PairedWith, source);
    response
}

fn split_method_parts(full_method: &str) -> (&str, &str) {
    if let Some((service, method)) = full_method.rsplit_once('.') {
        (service, method)
    } else {
        ("", full_method)
    }
}
