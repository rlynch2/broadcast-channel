use broadcast_channel::broadcaster;
use std::iter;
use std::thread;
#[test]
fn ten_senders() {
    let thread_num = 10;
    let nums_pushed = 10000;
    let (tx, mut rx) = broadcaster();
    let mut threads = Vec::new();
    for _ in 0..thread_num {
        let tx = tx.clone();
        let thread = thread::spawn(move || {
            for _ in 0..nums_pushed {
                tx.send(1);
            }
        });
        threads.push(thread);
    }
    for thread in threads {
        thread.join().unwrap();
    }

    for _ in 0..(thread_num * nums_pushed) {
        assert_eq!(rx.next(), Some(1));
    }
}

#[test]
fn ten_senders_ten_receivers() {
    let thread_num = 10;
    let nums_pushed = 10000;
    let (tx, rx) = broadcaster();
    let mut threads = Vec::new();

    let mut rxs = Vec::new();
    for _ in 0..thread_num {
        rxs.push(rx.clone());
    }
    for _ in 0..thread_num {
        let tx = tx.clone();
        let thread = thread::spawn(move || {
            tx.send_all(iter::repeat(1).take(nums_pushed));
        });
        threads.push(thread);
    }
    for thread in threads {
        thread.join().unwrap();
    }

    let mut threads = Vec::new();
    for mut rx in rxs {
        let thread = thread::spawn(move || {
            for i in 0..(thread_num * nums_pushed) {
                assert_eq!(rx.next(), Some(1), "Only {} passed!", i);
            }
        });
        threads.push(thread);
    }
    for thread in threads {
        thread.join().unwrap();
    }
}
