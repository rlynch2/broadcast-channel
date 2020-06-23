use broadcast_channel::*;

use std::sync::atomic::{AtomicU32, Ordering};

#[derive(Clone)]
struct SideEffectDrop(u32, &'static AtomicU32);
impl Drop for SideEffectDrop {
    fn drop(&mut self) {
        self.1.fetch_add(1, Ordering::SeqCst);
    }
}

#[test]
fn dropping() {
    static DROPS: AtomicU32 = AtomicU32::new(0);
    let (tx, rx) = broadcaster();
    for _ in 0..10 {
        tx.send(SideEffectDrop(1, &DROPS));
    }
    drop(tx);
    drop(rx);
    assert_eq!(DROPS.load(Ordering::SeqCst), 10);
}

#[test]
fn dropping_after_recv() {
    static DROPS: AtomicU32 = AtomicU32::new(0);
    let (tx, mut rx) = broadcaster();
    for _ in 0..10 {
        tx.send(SideEffectDrop(1, &DROPS));
    }

    rx.nth(2);

    drop(rx);
    drop(tx);
    // We drop the 10 put in the channel + 3 that are read from the receiver
    assert_eq!(DROPS.load(Ordering::SeqCst), 13);
}
