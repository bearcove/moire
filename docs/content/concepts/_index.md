+++
title = "Concepts"
weight = 1
sort_by = "weight"
insert_anchor_links = "heading"
+++

When a synchronous Rust application is stuck on something, chances are you
can attach a debugger and look at backtraces of all the threads and see exactly
what's happening.

When an _asynchronous_ Rust application (using tokio, for example) is stuck,
things are not so simple. What most likely happened, is that one of your futures
polled another future, which polled another future.

And that leaf future needed some work to be done that wasn't ready just now. So
it asked to be woken up later and then returned `Poll::Pending`. All the
futures that were currently being pulled also returned `Poll::Pending` all the way
to the runtime.

And when you, the developer, attach a debugger to the app, all you see is that
all the threads are parked waiting for something to happen. 

Well, if every feature of every library that you ever used was instrumented in
some way, you could tell who's waiting for what.

The `peeps::peeps!` macro lets you do that — Every future becomes a node in the
peeps graph and every `await` (or poll) becomes a 'needs' edge between the two:

This is already interesting, but it's extremely noisy and really hard to read.

peeps goes one step further by instrumenting synchronization primitives: If you
know every future that's currently waiting for a lock, and you know the future
that's currently holding the lock, then that's extremely helpful.

And similarly for channels, you can record properties of the channel itself,
such as how many values are currently buffered? How many subscribers are there
to a channel? You can record events with times. When did the last few items go
through? When did we stop getting items? 

And finally, because I'm the one writing this tool and I have my own ecosystem
including RPC via [roam](https://github.com/bearcove/roam), request and
responses are also instrumented, which lets you join multiple processes into a
single graph and understand cross-process deadlocks. 

At least that's the idea. In practice, it's really hard to build a tool like
this and even harder to use a tool like this to actually find real bugs. There
are toy examples in the repository that are extremely obvious, but everything is
still in flux. And at the time of this writing, the deadlock that I am chasing
is still alive and well.

So someone remind me to update these docs when I actually find it. 
 
<p style="text-align:center; margin-top:2rem;">
  <a class="cta-button" href="/instrumentation/">Go to Instrumentation</a>
</p>
