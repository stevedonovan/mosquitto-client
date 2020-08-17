extern crate mosquitto_client as mosq;
use mosq::Mosquitto;
use std::thread;

fn run() -> mosq::Result<()> {
    let m = Mosquitto::new("test");

    m.connect_wait("localhost",1883,5,300)?;
    let bilbo = m.subscribe("bilbo/#",1)?;

    let mt = m.clone();
    thread::spawn(move || {
        for i in 0..5 {
            let topic = format!("bilbo/{}",10*(i+1));
            let data = format!("hello #{}",i);
            mt.publish(&topic,data.as_bytes(), 1, false).unwrap();
        }
    });

    let msgs = bilbo.receive_many(300)?;
    for msg in msgs {
        println!("topic {} text '{}'",msg.topic(),msg.text());
    }
    Ok(())
}

fn main() {
    run().expect("error");
}
