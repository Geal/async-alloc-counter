//! async-alloc-counter measures max allocations in a future invocation
//!
//! see `examples/` for usage
use pin_project_lite::pin_project;
use std::{
    alloc::{GlobalAlloc, Layout},
    cell::RefCell,
    future::Future,
    pin::Pin,
    task::{Context, Poll},
    thread_local,
};

pub struct AsyncAllocatorCounter<T> {
    pub allocator: T,
}

unsafe impl<T: GlobalAlloc> GlobalAlloc for AsyncAllocatorCounter<T> {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        CurrentFrame.with(|frame| {
            if let Some(f) = (*frame.borrow_mut()).as_mut() {
                f.current += layout.size();
                if f.current > f.max {
                    f.max = f.current;
                }
            }
        });

        self.allocator.alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        CurrentFrame.with(|frame| {
            if let Some(f) = (*frame.borrow_mut()).as_mut() {
                f.current -= layout.size();
            }
        });

        self.allocator.dealloc(ptr, layout)
    }
}

pub trait TraceAlloc: Sized {
    fn trace(self, id: u32) -> TraceAllocator<Self> {
        TraceAllocator {
            inner: self,
            id,
            previous: None,
            max: 0,
            current: 0,
        }
    }
}

impl<T: Sized> TraceAlloc for T {}

pin_project! {
    pub struct TraceAllocator<T> {
        #[pin]
        inner: T,
        id: u32,
        max: usize,
        current: usize,
        previous: Option<AllocationFrame>,
    }
}

thread_local! {
    static CurrentFrame: RefCell<Option<AllocationFrame>> = RefCell::new(None);
}

impl<T: Future> Future for TraceAllocator<T> {
    type Output = (usize, T::Output);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();

        assert!(this.previous.is_none());
        let current = Some(AllocationFrame {
            max: *this.max,
            current: *this.current,
        });
        *this.previous = CurrentFrame.with(|frame| {
            let prev = (*frame.borrow_mut()).take();
            *frame.borrow_mut() = current;
            prev
        });
        let res = this.inner.poll(cx);
        let previous = this.previous.take();
        if let Some(AllocationFrame { max, current }) = CurrentFrame.with(|frame| {
            let current = (*frame.borrow_mut()).take();
            *frame.borrow_mut() = previous;
            current
        }) {
            if max > *this.max {
                *this.max = max;
            }
            *this.current = current;

            //println!("after: max={}, current={}", *this.max, *this.current);
        }

        match res {
            Poll::Pending => Poll::Pending,
            Poll::Ready(value) => Poll::Ready((*this.max, value)),
        }
    }
}

struct AllocationFrame {
    max: usize,
    current: usize,
}
