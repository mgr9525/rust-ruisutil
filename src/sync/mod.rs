mod waker;
mod wakers;

pub use waker::Waker;
pub use wakers::WakerFut;
pub use wakers::WakerOneFut;

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
    #[test]
    fn waker() {
        let wkr = crate::sync::Waker::new(&crate::Context::background(None));
        let wkrc = wkr.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_secs(3));
            wkrc.notify_all();
            std::thread::sleep(Duration::from_secs(5));
            wkrc.notify_one();
        });

        let now = SystemTime::now();
        println!("start wait");
        wkr.wait_timeout(Duration::from_millis(100));
        println!("end wait:{}ms", crate::tms_now_since(now).as_millis());

        let now = SystemTime::now();
        println!("start wait2");
        wkr.wait_timeout(Duration::from_millis(100));
        println!("end wait:{}ms", crate::tms_now_since(now).as_millis());
    }

    #[cfg(feature = "asyncs")]
    #[test]
    fn waker_fut() {
        async_std::task::block_on(async move {
            let wkr = crate::sync::WakerFut::new(&crate::Context::background(None));
            let wkrc = wkr.clone();
            async_std::task::spawn(async move {
                async_std::task::sleep(Duration::from_secs(5)).await;
                wkrc.notify_all();
                async_std::task::sleep(Duration::from_secs(5)).await;
                wkrc.notify_one();
                async_std::task::sleep(Duration::from_secs(5)).await;
                wkrc.close();
            });

            let now = SystemTime::now();
            let wkrc = wkr.clone();
            async_std::task::spawn(async move {
                println!("start wait1");
                async_std::io::timeout(Duration::from_secs(2), wkrc).await;
                println!("end wait1:{}ms", crate::tms_now_since(now).as_millis());
            });

            println!("start wait");
            wkr.clone().await;
            println!("end wait:{}ms", crate::tms_now_since(now).as_millis());

            let now = SystemTime::now();
            println!("start wait2");
            wkr.clone().await;
            println!("end wait2:{}ms", crate::tms_now_since(now).as_millis());

            let now = SystemTime::now();
            println!("start wait3");
            wkr.await;
            println!("end wait3:{}ms", crate::tms_now_since(now).as_millis());
        });
    }
}
