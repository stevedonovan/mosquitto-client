extern crate mosquitto_client as mosq;
use mosq::Mosquitto;

fn main() {
    let m = Mosquitto::new("test");

    m.will_set("test/will",b"finished!",0,false).expect("can't set will");

    m.connect("localhost",1883,5).expect("can't connect");
    let bonzo = m.subscribe("bonzo/#",0).expect("can't subscribe to bonzo");
    let frodo = m.subscribe("frodo/#",0).expect("can't subscribe to frodo");

    // not interested in any retained messages!
    let mut mc = m.callbacks(());
    mc.on_message(|_,msg| {
        if ! msg.retained() {
            if bonzo.matches(&msg) {
                println!("bonzo {:?}",msg);
            } else
            if frodo.matches(&msg) {
                println!("frodo {:?}",msg);
                m.disconnect().unwrap();
            }
        }
    });

    m.loop_forever(200).expect("broken loop");
}
