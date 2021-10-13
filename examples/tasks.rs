use async_alloc_counter::*;
use futures::FutureExt;
use std::{alloc::System, time::Duration};

#[global_allocator]
static GLOBAL: AsyncAllocatorCounter<System> = AsyncAllocatorCounter { allocator: System };

#[tokio::main]
async fn main() {
    for i in 1..100 {
        let f = async move {
            let mut v = Vec::new();

            for j in 0..i {
                v.push(j);
                tokio::task::spawn(
                    async move {
                        let s = format!("in task {}-{}", i, j);
                        println!("{}", s);
                        tokio::time::sleep(Duration::from_millis(100)).await;
                    }
                    .trace()
                    .map(move |(max, ())| {
                        println!("inner task {} allocated {} max bytes", i * 1000 + j, max);
                    }),
                )
                .await
                .unwrap();
            }
        }
        .trace()
        .map(move |(max, ())| {
            println!("task {} allocated {} max bytes", i, max);
        });

        tokio::task::spawn(f);
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
