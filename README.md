# moire (mwah-ray)

runtime graph instrumentation for tokio-based Rust systems.

moiré replaces Tokio's primitives with named, instrumented wrappers. At every
API boundary — every lock acquisition, channel send/receive, spawn, and RPC call
— it captures the current call stack via frame-pointer walking.

The resulting graph of entities (tasks, locks, channels, RPC calls) connected by
typed edges (polls, waiting_on, paired_with, holds) is pushed as a live stream
to `moire-web` for investigation.

![Image](https://github.com/user-attachments/assets/1600e7d1-9cb2-49ce-a74d-60205a6930e9)

The dashboard shows you which tasks are stuck, what they are waiting on, and
exactly where in your code they got there.

![Image](https://github.com/user-attachments/assets/57f0c3be-f446-4bbd-9a1b-e013b603ccac)
