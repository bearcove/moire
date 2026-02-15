# Resource Track: Sockets

Status: todo
Owner: wg-resource-sockets
Priority: P0

## Mission

Instrument socket waits and usage explicitly so network stalls are visible in causal chains.

## Current context

- Some socket references currently appear as labels/resources but not full canonical nodes/edges.
- Roam is the best first target because socket usage is concentrated there.

## Node + edge model

Node ID:
- `socket:{process}:{fd}`

Node kind:
- `socket`

Required attrs_json:
- `fd`
- `label`
- `peer`
- `proto`
- `last_read_ns`
- `last_write_ns`

Required edges:
- `task_waits_on_socket_read` (`task -> socket`) with wait duration
- `task_waits_on_socket_write` (`task -> socket`) with wait duration
- `request_uses_socket` (`request -> socket`) when request/task/socket linkage is explicit

## Implementation steps

1. Add socket wrapper types for roam (`TcpStream`, read/write helpers).
2. Instrument read/write await points for blocked durations.
3. Emit socket nodes with stable FD identity.
4. Emit request/socket edges only where request context is explicitly available.

## Consumer changes

Required:
- Replace raw `tokio::net::*` usage with peeps socket wrappers in roam first.
- Expand to Vixen callsites after roam coverage is stable.

## Validation SQL

```sql
SELECT kind, COUNT(*)
FROM edges
WHERE snapshot_id = ?1 AND kind LIKE 'task_waits_on_socket_%'
GROUP BY kind;
```
