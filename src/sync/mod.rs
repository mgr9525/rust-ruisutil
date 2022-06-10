
mod waker;

pub use waker::Waker;



#[cfg(test)]
mod tests {
    use std::time::{Duration, SystemTime};


  #[test]
  fn it_works() {
      assert_eq!(2 + 2, 4);
  }
  #[test]
  fn waker() {
      async_std::task::block_on(async move{
        let wkr=crate::sync::Waker::new();
        let wkrc=wkr.clone();
        async_std::task::spawn(async move{
          async_std::task::sleep(Duration::from_secs(5)).await;
          // wkrc.notify_one().await;
          wkrc.notify_all().await;
        });

        let now=SystemTime::now();
        println!("start wait");
        wkr.wait().await;
        if let Ok(v)=SystemTime::now().duration_since(now){
          println!("end wait:{}ms",v.as_millis());
        }else{
          println!("end wait");
        }
      });
  }
}