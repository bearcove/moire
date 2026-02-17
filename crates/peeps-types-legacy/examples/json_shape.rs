use peeps_types::{Edge, EdgeKind, Event, GraphReply, GraphSnapshot, Node, NodeKind};

fn main() {
    let reply = GraphReply {
        r#type: "graph_reply".to_string(),
        snapshot_id: 123,
        process: "swift-vixenfs".to_string(),
        pid: 42,
        graph: Some(GraphSnapshot {
            process_name: "swift-vixenfs".to_string(),
            proc_key: "swift-vixenfs-42".to_string(),
            nodes: vec![Node {
                id: "request:abc".to_string(),
                kind: NodeKind::Request,
                label: Some("req".to_string()),
                attrs_json: "{}".to_string(),
            }],
            edges: vec![Edge {
                src: "request:abc".to_string(),
                dst: "response:def".to_string(),
                kind: EdgeKind::Needs,
                attrs_json: "{}".to_string(),
            }],
            events: Some(vec![Event {
                id: "event:abc".to_string(),
                ts_ns: 1_734_000_000_000_000_000,
                proc_key: "swift-vixenfs-42".to_string(),
                entity_id: "request:abc".to_string(),
                name: "request.started".to_string(),
                parent_entity_id: None,
                attrs_json: "{\"phase\":\"begin\"}".to_string(),
            }]),
        }),
    };
    let json = facet_json::to_string(&reply).unwrap();
    println!("{}", json);
    let decoded: GraphReply = facet_json::from_slice(json.as_bytes()).unwrap();
    let decoded_graph = decoded.graph.unwrap();
    assert_eq!(decoded_graph.events.as_ref().map(Vec::len), Some(1));
    println!(
        "decoded kind={} edge={} events={}",
        decoded_graph.nodes[0].kind.as_str(),
        reply.graph.unwrap().edges[0].kind.as_str(),
        decoded_graph.events.unwrap()[0].name
    );

    // Backward-compat decode: payload without `events` still decodes.
    let no_events_json = r#"{
  "type":"graph_reply",
  "snapshot_id":124,
  "process":"swift-vixenfs",
  "pid":42,
  "graph":{
    "process_name":"swift-vixenfs",
    "proc_key":"swift-vixenfs-42",
    "nodes":[],
    "edges":[]
  }
}"#;
    let decoded_no_events: GraphReply = facet_json::from_slice(no_events_json.as_bytes()).unwrap();
    assert!(decoded_no_events.graph.unwrap().events.is_none());
}
