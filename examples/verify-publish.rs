extern crate mosquitto_client as mosq;
use mosq::Mosquitto;

use std::error::Error;

fn go() -> Result<(),Box<Error>> {
    let m = Mosquitto::new("test");
    
    m.connect("localhost",1883)?;

    let our_mid = m.publish("bonzo/dog","hello dolly".as_bytes(), 2, false)?;
    
    let mut mc = m.callbacks(());
    mc.on_publish(|_,mid| {
        if mid == our_mid {
            m.disconnect().unwrap();
        }
    });

    m.loop_until_disconnect(-1).expect("loop problem");
    Ok(())
}

fn main() {
    go().expect("error: ");
}
