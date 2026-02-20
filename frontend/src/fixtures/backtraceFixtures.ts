import type { SnapshotBacktrace } from "../api/types.generated";

export const STORYBOOK_BACKTRACE_FIXTURES: SnapshotBacktrace[] = [
  {
    backtrace_id: 9001,
    frames: [
      {
        resolved: {
          module_path: "moire_examples::roam::work_queue",
          function_name: "blocked_receiver",
          source_file: "/Users/amos/bearcove/moire/crates/moire-examples/src/roam/work_queue.rs",
          line: 117,
        },
      },
      {
        resolved: {
          module_path: "moire_examples::roam::work_queue",
          function_name: "spawn_demo",
          source_file: "/Users/amos/bearcove/moire/crates/moire-examples/src/roam/work_queue.rs",
          line: 83,
        },
      },
      {
        resolved: {
          module_path: "tokio::runtime::task::harness",
          function_name: "poll_future",
          source_file: "/rustc/1.84.0/library/tokio/src/runtime/task/harness.rs",
          line: 496,
        },
      },
      {
        resolved: {
          module_path: "tokio::runtime::scheduler::multi_thread::worker",
          function_name: "run",
          source_file: "/rustc/1.84.0/library/tokio/src/runtime/scheduler/multi_thread/worker.rs",
          line: 523,
        },
      },
      {
        unresolved: {
          module_path: "/usr/lib/system/libsystem_pthread.dylib",
          rel_pc: 74520,
          reason: "debug symbols unavailable",
        },
      },
    ],
  },
  {
    backtrace_id: 9002,
    frames: [
      {
        resolved: {
          module_path: "moire_web::snapshot",
          function_name: "convert_snapshot",
          source_file: "/Users/amos/bearcove/moire/frontend/src/snapshot.ts",
          line: 582,
        },
      },
      {
        unresolved: {
          module_path: "/Users/amos/bearcove/moire/target/debug/moire-web",
          rel_pc: 402341,
          reason: "pc not covered by debug line table",
        },
      },
      {
        resolved: {
          module_path: "std::rt::lang_start_internal",
          function_name: "lang_start_internal",
          source_file: "/rustc/1.84.0/library/std/src/rt.rs",
          line: 171,
        },
      },
    ],
  },
];
