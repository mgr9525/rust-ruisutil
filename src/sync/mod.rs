mod waker;
mod wakers;

pub use waker::Waker;
pub use wakers::WakerFut;

#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};

    #[test]
    fn it_works() {
        assert_eq!(2 + 2, 4);
    }
    #[test]
    fn waker() {
        async_std::task::block_on(async move {
            let wkr = crate::sync::Waker::new();
            let wkrc = wkr.clone();
            async_std::task::spawn(async move {
                async_std::task::sleep(Duration::from_secs(3)).await;
                wkrc.notify_all().await;
                async_std::task::sleep(Duration::from_secs(5)).await;
                wkrc.notify_one().await;
            });

            let now = SystemTime::now();
            println!("start wait");
            wkr.wait().await;
            if let Ok(v) = SystemTime::now().duration_since(now) {
                println!("end wait:{}ms", v.as_millis());
            } else {
                println!("end wait");
            }

            let now = SystemTime::now();
            println!("start wait2");
            wkr.wait().await;
            if let Ok(v) = SystemTime::now().duration_since(now) {
                println!("end wait2:{}ms", v.as_millis());
            } else {
                println!("end wait2");
            }
        });
    }
    #[test]
    fn wakerFut() {
        async_std::task::block_on(async move {
            let wkr = crate::sync::WakerFut::new();
            let wkrc = wkr.clone();
            async_std::task::spawn(async move {
                async_std::task::sleep(Duration::from_secs(5)).await;
                wkrc.notify_all();
                async_std::task::sleep(Duration::from_secs(5)).await;
                wkrc.notify_one();
            });

            let now = SystemTime::now();
            let wkrc = wkr.clone();
            async_std::task::spawn(async move {
                println!("start wait1");
                wkrc.await;
                if let Ok(v) = SystemTime::now().duration_since(now) {
                    println!("end wait1:{}ms", v.as_millis());
                } else {
                    println!("end wait1");
                }
            });

            println!("start wait");
            wkr.clone().await;
            if let Ok(v) = SystemTime::now().duration_since(now) {
                println!("end wait:{}ms", v.as_millis());
            } else {
                println!("end wait");
            }

            let now = SystemTime::now();
            println!("start wait2");
            wkr.await;
            if let Ok(v) = SystemTime::now().duration_since(now) {
                println!("end wait2:{}ms", v.as_millis());
            } else {
                println!("end wait2");
            }
        });
    }
}
