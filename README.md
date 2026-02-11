# peeps

Instrumentation for:

  * parking_lot locks
  * tokio locks
  * tokio tasks (via spawn)
  
Also:

  * thread dumps (sampled a few times)
  * roam RPC state (connections, in-flight requests, control flow etc.)
  
Dumped to a dir on SIGUSR1 as a bunch of JSON, parsed, served on a web browser.
