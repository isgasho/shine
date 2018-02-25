extern crate shine_store;

use std::thread;
use std::sync::Arc;
use std::env;

use self::shine_store::fjsqueue::*;
use self::shine_store::threadid;


#[test]
fn consume()
{
    #[derive(Debug)]
    struct Data(usize);

    impl Data {
        fn new(i: usize) -> Data {
            Data(i)
        }
    }

    let store = FJSQueue::<u16, Data>::new();

    // insert some elements than drain them
    {
        let mut store = store.produce();
        store.add(0, Data::new(0));
        store.add(2, Data::new(2));
        store.add(1, Data::new(1));
    }
    {
        let mut store = store.consume(|&k| k as u64);
        for (i, d) in store.drain().enumerate() {
            assert_eq!(d.0, i);
        }
    }

    // insert again some more
    {
        {
            let mut store = store.produce();
            for i in 0..1024 {
                store.add(100 + i, Data::new(100 + i as usize));
            }
        }
        {
            let mut store = store.consume(|&k| k as u64);
            for (i, d) in store.drain().enumerate() {
                assert_eq!(d.0, 100 + i);
            }
        }
    }
}


#[test]
fn simple()
{
    let store = Arc::new(FJSQueue::<u16, (u16, usize, usize)>::new());

    let mut tp = Vec::new();

    for tid in 0..threadid::get_max_thread_count() {
        let store = store.clone();
        tp.push(
            thread::spawn(move || {
                let mut store = store.produce();
                store.add(20, (20, tid, 0));
                store.add(23, (23, tid, 1));
                store.add(21, (21, tid, 2));
                store.add(23, (23, tid, 3));
                store.add(12, (12, tid, 4));
                store.add(23, (23, tid, 5));
                store.add(23, (23, tid, 6));
                store.add(24, (24, tid, 7));
                store.add(23, (23, tid, 8));
                store.add(10, (10, tid, 9));
            }));
    }

    for t in tp.drain(..) {
        t.join().unwrap();
    }

    {
        let mut store = store.consume(|&k| k as u64);
        let mut drain = store.drain();
        let mut prev = drain.next().unwrap();
        //println!("data[{}] = {:?}", 0, prev);
        for (_i, d) in drain.enumerate() {
            //println!("data[{}] = {:?}", i + 1, d);
            assert!(prev.0 <= d.0);
            assert!(prev.0 != d.0 || prev.1 != d.1 || prev.2 < d.2, "sort is not stable");
            prev = d;
        }
    }
}

#[test]
fn check_lock() {
    // single threaded as panic hook is a global resource
    assert!(env::var("RUST_TEST_THREADS").unwrap_or("0".to_string()) == "1", "This test shall run in single threaded test environment: RUST_TEST_THREADS=1");

    use std::mem;
    use std::panic;

    // create a newtype to have RefUnwindSafe property for the queue
    struct Queue(FJSQueue<u16, (u16, usize, usize)>);
    impl panic::RefUnwindSafe for Queue {}

    panic::set_hook(Box::new(|_info| { /*println!("panic: {:?}", _info);*/ }));

    {
        let store = Queue(FJSQueue::<u16, (u16, usize, usize)>::new());
        assert!(panic::catch_unwind(|| {
            let p0 = store.0.produce();
            let p1 = store.0.produce();
            drop(p1);
            drop(p0);
        }).is_err());
        mem::forget(store);
    }

    {
        let store = Queue(FJSQueue::<u16, (u16, usize, usize)>::new());
        assert!(panic::catch_unwind(|| {
            let p0 = store.0.produce();
            let p1 = store.0.consume(|&k| k as u64);
            drop(p1);
            drop(p0);
        }).is_err());
        mem::forget(store);
    }

    {
        let store = Queue(FJSQueue::<u16, (u16, usize, usize)>::new());
        assert!(panic::catch_unwind(|| {
            let p0 = store.0.consume(|&k| k as u64);
            let p1 = store.0.produce();
            drop(p1);
            drop(p0);
        }).is_err());
        mem::forget(store);
    }

    {
        let store = Queue(FJSQueue::<u16, (u16, usize, usize)>::new());
        assert!(panic::catch_unwind(|| {
            let p0 = store.0.consume(|&k| k as u64);
            let p1 = store.0.consume(|&k| k as u64);
            drop(p1);
            drop(p0);
        }).is_err());
        mem::forget(store);
    }

    panic::take_hook();
}
