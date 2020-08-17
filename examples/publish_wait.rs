extern crate mosquitto_client as mosq;
use mosq::Mosquitto;
use std::time::{Instant,Duration};

// you would think that the stdlib would actually provide
// a method to do this...
fn as_millis(d: Duration) -> f64 {
    1000.0*(d.as_secs() as f64) + (d.subsec_nanos() as f64)/1e6
}

const TIMEOUT: i32 = 300;

fn run() -> Result<(),Box<dyn std::error::Error>> {
    let m = Mosquitto::new("test");

    let t = Instant::now();

    m.connect_wait("localhost",1883,5,TIMEOUT)?;
    m.publish_wait("/bonzo/dog",b"hello dolly",2,false,TIMEOUT)?;
    m.publish_wait("/bonzo/cat",b"meeeaaaww",2,false,TIMEOUT)?;
    println!("elapsed {:.2} msec",as_millis(t.elapsed()));
    Ok(())
}

fn main() {
    run().expect("failed");
}
