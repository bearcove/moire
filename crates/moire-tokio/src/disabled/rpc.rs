/// No-op RPC request handle for the disabled (no-instrumentation) backend.
#[derive(Clone)]
pub struct RpcRequestHandle {
    id: String,
}

impl RpcRequestHandle {
    pub fn id_for_wire(&self) -> String {
        self.id.clone()
    }
}

/// No-op RPC response handle for the disabled backend.
#[derive(Clone)]
pub struct RpcResponseHandle;

pub fn rpc_request(_method: impl Into<String>, _args_json: impl Into<String>) -> RpcRequestHandle {
    RpcRequestHandle { id: String::new() }
}

pub fn rpc_response(_method: impl Into<String>) -> RpcResponseHandle {
    RpcResponseHandle
}

pub fn rpc_response_for(
    _method: impl Into<String>,
    _request: &RpcRequestHandle,
) -> RpcResponseHandle {
    RpcResponseHandle
}
