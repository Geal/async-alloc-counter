async-alloc-counter measures max allocations in a future invocation

see `examples/` for usage

This allocator can be used as follows:

```rust
use async_alloc_counter::*;
use futures::FutureExt;
use std::{alloc::System, time::Duration};

// set up the counting allocator
#[global_allocator]
static GLOBAL: AsyncAllocatorCounter<System> = AsyncAllocatorCounter { allocator: System };

#[tokio::main]
async fn main() {
  async move {
    let mut v: Vec<u8> = Vec::with_capacity(1024);
  }.count_allocations()
   .map(move |(max, ())| {
     println!("future allocated {} max bytes",  max);
   })
   .await
}
```

Allocation measurement can be stacked:

```rust,ignore
async move {
  println!("wrapping future");
  tokio::time::sleep(std::timeDuration::from_secs(1)).await;
  let mut v: Vec<u8> = Vec::with_capacity(256);

  async move {
      let mut v: Vec<u8> = Vec::with_capacity(1024);
    }.count_allocations()
     .map(move |(max, ())| {
       println!("future allocated {} max bytes",  max);
     })
     .await
  }.count_allocations()
   .map(move |(max, ())| {
     println!("warpping future allocated {} max bytes",  max);
   })
   .await
```

Design inspired by the excellent [tracing](https://crates.io/crates/tracing) crate