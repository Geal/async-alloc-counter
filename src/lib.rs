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
    fn trace(self) -> TraceAllocator<Self> {
        TraceAllocator {
            inner: self,
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
            // we want to know the maximum allocation in this current call of the
            // future, we start at the currently allocated value
            max: *this.current,
            current: *this.current,
        });

        // store the allocation frame from upper layers if there was one
        *this.previous = CurrentFrame.with(|frame| {
            let prev = (*frame.borrow_mut()).take();
            *frame.borrow_mut() = current;
            prev
        });

        let res = this.inner.poll(cx);

        let mut previous = this.previous.take();
        if let Some(AllocationFrame { max, current }) = CurrentFrame.with(|frame| {
            let current = (*frame.borrow_mut()).take();

            if let Some(prev) = previous.as_mut() {
                if let Some(f) = current.as_ref() {
                    // prev.current contains the total current allocation in the
                    // previous frame, including allocations in the current frame
                    // to chek if we can raise the max in the previous frame,
                    // we need to substrat this.current, which was already
                    // integrated in prev.current, and add the max allocations
                    // seen in the current invocation
                    if prev.current - *this.current + f.max > prev.max {
                        prev.max = prev.current - *this.current + f.max;
                    }

                    if f.current > *this.current {
                        prev.current += f.current - *this.current;
                    } else {
                        prev.current -= *this.current - f.current;
                    }
                }
            }

            *frame.borrow_mut() = previous;
            current
        }) {
            if max > *this.max {
                *this.max = max;
            }
            *this.current = current;
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
