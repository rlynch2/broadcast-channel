//! Channels to broadcast messages to all their receivers

use std::marker::PhantomData;
use std::ptr;
use std::sync::atomic::{AtomicPtr, AtomicUsize, Ordering};
use std::sync::Arc;
use std::thread;

/// Creates a [`Sender`] and [`Receiver`] to broadcast messages.
/// See the module level documentation for more info.
///
/// [`Sender`]: struct.Sender.html
/// [`Receiver`]: struct.Receiver.html
pub fn broadcaster<T: Send + Clone + 'static>() -> (Sender<T>, Receiver<T>) {
    let channel = Arc::new(BroadcastChannel::new());
    (Sender::new(channel.clone()), Receiver::new(channel))
}

struct BroadcastChannel<T: Send + Clone> {
    head: AtomicPtr<Node<T>>,
    tail: AtomicPtr<Node<T>>,
    readers: AtomicUsize,
    _data: PhantomData<T>, //hazards: Map<*mut T, AtomicUsize>
}

impl<T: Send + Clone + 'static> BroadcastChannel<T> {
    fn new() -> Self {
        let channel = Self {
            head: AtomicPtr::new(ptr::null_mut()),
            tail: AtomicPtr::new(ptr::null_mut()),
            // We create the channel with 1 sender so we can just
            // set the `readers` to 1 to avoid an additional atomic write
            readers: AtomicUsize::new(0),
            _data: PhantomData,
        };
        let first = Box::into_raw(Box::new(Node {
            value: None,
            next: AtomicPtr::new(ptr::null_mut()),
            readers: AtomicUsize::new(1),
        }));
        channel.head.store(first, Ordering::SeqCst);
        channel.tail.store(first, Ordering::SeqCst);
        channel
    }

    fn send(&self, value: T) {
        let node = Box::into_raw(Box::new(Node::<T> {
            value: Some(value),
            next: AtomicPtr::new(ptr::null_mut()),
            readers: AtomicUsize::new(self.readers.load(Ordering::SeqCst)),
        }));

        loop {
            let head = self.head.load(Ordering::SeqCst);

            let old_head = self.head.compare_and_swap(head, node, Ordering::SeqCst);
            if old_head != head {
                thread::yield_now();
                continue;
            }

            // TODO: Add a hazard pointer to fix a possible data race between this value being dropped before we set the next pointer
            if !head.is_null() {
                // SAFETY: The head always be pointing at a valid node
                let old_node = unsafe { &*head };
                // No other threads should be changing this node after we switch the head
                assert_ne!(
                    old_node
                        .next
                        .compare_and_swap(ptr::null_mut(), node, Ordering::SeqCst),
                    node
                );
            }
            break;
        }
    }
}

impl<T: Send + Clone> Drop for BroadcastChannel<T> {
    fn drop(&mut self) {
        while !(*self.tail.get_mut()).is_null() {
            let tail = *self.tail.get_mut();
            unsafe {
                *self.tail.get_mut() = *(*tail).next.get_mut();

                ptr::drop_in_place(tail);
            }
        }
    }
}

struct Node<T: Send + Clone> {
    value: Option<T>,
    next: AtomicPtr<Node<T>>,
    readers: AtomicUsize,
}

#[derive(Clone)]
pub struct Sender<T: Send + Clone> {
    channel: Arc<BroadcastChannel<T>>,
}

impl<T: Send + Clone + 'static> Sender<T> {
    fn new(channel: Arc<BroadcastChannel<T>>) -> Self {
        Self { channel }
    }

    pub fn send(&self, value: T) {
        self.channel.send(value);
    }

    pub fn send_all<I: IntoIterator<Item = T>>(&self, into_iter: I) {
        for item in into_iter {
            self.send(item);
        }
    }
}

unsafe impl<T: Send + Clone> Send for Receiver<T> {}
pub struct Receiver<T: Send + Clone + 'static> {
    current: *mut Node<T>,
    channel: Arc<BroadcastChannel<T>>,
}

impl<T: Send + Clone> Receiver<T> {
    fn new(channel: Arc<BroadcastChannel<T>>) -> Self {
        Self {
            current: channel.head.load(Ordering::SeqCst),
            channel,
        }
    }
}

impl<T: Send + Clone + 'static> Clone for Receiver<T> {
    fn clone(&self) -> Self {
        let head = loop {
            let old_head = self.channel.head.load(Ordering::SeqCst);
            self.channel.readers.fetch_add(1, Ordering::SeqCst);
            if old_head != self.channel.head.load(Ordering::SeqCst) {
                self.channel.readers.fetch_sub(1, Ordering::SeqCst);
            } else {
                break old_head;
            }
        };

        Self {
            current: head,
            channel: self.channel.clone(),
        }
    }
}

impl<T: Send + Clone + 'static> Iterator for Receiver<T> {
    type Item = T;

    fn next(&mut self) -> Option<T> {
        let old = self.current;
        let current = unsafe { &*old }.next.load(Ordering::SeqCst);
        if current.is_null() {
            return None;
        }
        self.current = current;
        let value = unsafe { &*current }.value.clone();
        unsafe {
            if (*old).readers.fetch_sub(1, Ordering::SeqCst) == 1 {
                ptr::drop_in_place(old);
                self.channel
                    .tail
                    .compare_and_swap(old, current, Ordering::SeqCst);
            }
        }
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sending() {
        let (tx, mut rx) = broadcaster();
        tx.send(1);
        tx.send(2);

        assert_eq!(Some(1), rx.next());
        assert_eq!(Some(2), rx.next());
    }

    #[test]
    fn send_all() {
        let (tx, mut rx) = broadcaster();
        tx.send_all(0..1000);
        for i in 0..1000 {
            assert_eq!(rx.next(), Some(i));
        }
    }

    #[test]
    fn multiple_receivers() {
        let (tx, mut rx) = broadcaster();
        let mut rx2 = rx.clone();
        tx.send(1);
        tx.send(2);
        let mut rx3 = rx.clone();
        tx.send(3);
        assert_eq!(Some(1), rx.next());
        assert_eq!(Some(2), rx.next());
        assert_eq!(Some(3), rx.next());
        assert_eq!(None, rx.next());

        assert_eq!(Some(1), rx2.next());
        assert_eq!(Some(2), rx2.next());
        assert_eq!(Some(3), rx2.next());
        assert_eq!(None, rx2.next());

        assert_eq!(Some(3), rx3.next());
        assert_eq!(None, rx3.next());
    }

    #[test]
    fn multiple_senders() {
        let (tx, rx) = broadcaster();
        let tx2 = tx.clone();
        tx.send_all(0..5);
        tx2.send_all(5..10);
        for (i, item) in rx.enumerate() {
            assert_eq!(i, item as usize);
        }
    }
}
