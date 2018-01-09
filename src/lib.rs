//! Mosquitto is a popular MQTT broker implemented in C. Although there are pure
//! Rust MQTT clients, it is still useful to have a binding to the Mosquitto client.
//!
//! The basic story is that you connect to a broker, _subscribing_ to topics that
//! interest you and _publishing_ messages on a particular topic. The messages
//! may be any arbitrary bytes, but this implementation does require that the topics
//! themselves be UTF-8.  The C API is based on callbacks, which are mapped onto
//! Rust closures. Everything starts with [Mosquitto](struct.Mosquitto.html).
//!
//! For example, publishing a message and confirming that it is sent:
//!
//! ```rust
//! # fn run() -> std::result::Result<(),Box<std::error::Error>> {
//! let m = mosquitto_client::Mosquitto::new("test");
//!
//! m.connect("localhost",1883)?;
//!
//! // publish and get a message id
//! let our_mid = m.publish("bonzo/dog","hello dolly".as_bytes(), 2, false)?;
//!
//! // and wait for confirmation for that message id
//! let mut mc = m.callbacks(());
//! mc.on_publish(|_,mid| {
//!     if mid == our_mid {
//!         m.disconnect().unwrap();
//!     }
//! });
//!
//! // wait forever until explicit disconnect
//! // -1 means use default timeout on operations
//! m.loop_until_disconnect(-1)?;
//! # Ok(())
//! # }
//! #
//! # fn main() {
//! #    run().unwrap();
//! # }
//! ```
//!
//! Here we subscribe and listen to several topics:
//!
//! ```rust,no_run
//! # fn run() -> std::result::Result<(),Box<std::error::Error>> {
//! let m = mosquitto_client::Mosquitto::new("test");
//!
//! m.connect("localhost",1883)?;
//! let bonzo = m.subscribe("bonzo/#",0)?;
//! let frodo = m.subscribe("frodo/#",0)?;
//!
//! let mut mc = m.callbacks(());
//! mc.on_message(|_,msg| {
//!     if ! msg.retained() { // not interested in any retained messages!
//!         if bonzo.matches(&msg) {
//!             println!("bonzo {:?}",msg);
//!         } else
//!         if frodo.matches(&msg) {
//!             println!("frodo {:?}",msg);
//!         }
//!     }
//! });
//!
//! m.loop_forever(200)?;
//! # Ok(())
//! # }
//! #
//! # fn main() {
//! #    run().unwrap();
//! # }
//! ```
//!
//! You can always just do a regular match on the recevied topic name
//! from the [MosqMessage](struct.MosqMessage.html) `topic` method.
//!
//! The `callbacks` method can be given a value, and the _first_ argument of any
//! callback will be a mutable reference to that value (this avoids the usual
//! shenanigans involved with closures having mutable borrows)
//!
//! ```rust
//! # fn run() -> std::result::Result<(),Box<std::error::Error>> {
//! use std::{thread,time};
//!
//! let m = mosquitto_client::Mosquitto::new("test");
//!
//! m.connect("localhost",1883)?;
//! m.subscribe("bilbo/#",1)?;
//!
//! let mt = m.clone();
//! thread::spawn(move || {
//!     let timeout = time::Duration::from_millis(500);
//!     for i in 0..5 {
//!         let msg = format!("hello #{}",i+1);
//!         mt.publish("bilbo/baggins",msg.as_bytes(), 1, false).unwrap();
//!         thread::sleep(timeout);
//!     }
//!     mt.disconnect().unwrap();
//! });
//!
//! let mut mc = m.callbacks(Vec::new());
//! mc.on_message(|data,msg| {
//!     data.push(msg.text().to_string());
//! });
//!
//! m.loop_until_disconnect(200)?;
//! assert_eq!(mc.data.len(),5);
//! # Ok(())
//! # }
//! #
//! # fn main() {
//! #    run().unwrap();
//! # }
//! ```
//!
use std::os::raw::{c_int,c_char};
use std::ffi::{CStr,CString};
use std::error;
use std::fmt;
use std::time::{Duration,Instant};
use std::fmt::{Display,Debug};
use std::ptr::null;
use std::sync::atomic::{AtomicUsize, Ordering, ATOMIC_USIZE_INIT};

static INSTANCES: AtomicUsize = ATOMIC_USIZE_INIT;

pub mod sys;

use sys::*;

/// Our Error type.
/// Covers both regular Mosquitto errors and connection errors.
#[derive(Debug)]
pub struct Error {
    text: String,
    errcode: i32,
    connect: bool,
}

impl Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}",self.text)
    }
}

pub type Result<T> = ::std::result::Result<T,Error>;

impl Error {
    /// create a new Mosquitto error
    pub fn new(msg: &str, rc: c_int) -> Error {
        Error{text: format!("{}: {}",msg,mosq_strerror(rc)), errcode: rc, connect: false}
    }

    /// create a new connection error
    pub fn new_connect(rc: c_int) -> Error {
        Error{text: connect_error(rc).into(), errcode: rc, connect: true}
    }

    fn result(call: &str, rc: c_int) -> Result<()> {
        if rc != 0 {
            Err(Error::new(call,rc))
        } else {
            Ok(())
        }
    }

    /// underlying error code
    pub fn error(&self) -> i32 {
        self.errcode
    }
}

impl error::Error for Error {
    fn description(&self) -> &str {
        &self.text
    }
}

fn cs(s: &str) -> CString {
    CString::new(s).expect("Text contained nul bytes")
}

/// A mosquitto message
pub struct MosqMessage {
    msg: *const Message,
    owned: bool
}

use std::mem;

#[link(name = "c")]
extern {
    fn malloc(size: usize) -> *mut u8;
}

impl MosqMessage {
    fn new(msg: *const Message, clone: bool) -> MosqMessage {
        if clone {
            unsafe {
                let m = malloc(mem::size_of::<Message>()) as *mut Message;
                mosquitto_message_copy(m,msg);
                MosqMessage{msg:m,owned:true}
            }
        } else {
            MosqMessage{msg:msg, owned:false}
        }
    }

    fn msg_ref(&self) -> &Message {
        unsafe { &*self.msg }
    }

    /// the topic of the message.
    /// This will **panic** if the topic isn't valid UTF-8
    pub fn topic(&self) -> &str {
        unsafe { CStr::from_ptr(self.msg_ref().topic).to_str().expect("Topic was not UTF-8")  }
    }

    /// the payload as bytes
    pub fn payload(&self) -> &[u8] {
        let msg = self.msg_ref();
        unsafe {
            ::std::slice::from_raw_parts(
                msg.payload,
                msg.payloadlen as usize
            )
        }
    }

    /// the payload as text.
    /// This will **panic** if the payload was not valid UTF-8
    pub fn text(&self) -> &str {
        ::std::str::from_utf8(self.payload()).expect("Payload was not UTF-8")
    }

    /// the quality-of-service of the message.
    /// The desired QoS is specified when we subscribe.
    pub fn qos(&self) -> u32 {
        self.msg_ref().qos as u32
    }

    /// was the message retained by the broker?
    /// True if we received this as a retained message.
    /// Subsequent messages marked as retained will not set this.
    pub fn retained(&self) -> bool {
        if self.msg_ref().retain > 0 {true} else {false}
    }
}

impl Debug for MosqMessage {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let this = self.msg_ref();
        write!(f,"{}: mid {} len {} qos {} retain {}",self.topic(),
            this.mid, this.payloadlen, this.qos, this.retain)
    }
}

impl Clone for MosqMessage {
    fn clone(&self) -> Self {
        MosqMessage::new(self.msg,true)
    }
}

impl Drop for MosqMessage {
    fn drop(&mut self) {
        // eprintln!("dropping {}",self.owned);
        if self.owned {
            unsafe { mosquitto_message_free(&self.msg) };
        }
    }
}

/// Matching subscription topics.
/// Returned from [Mosquitto::subscribe](struct.Mosquitto.html#method.subscribe).
pub struct TopicMatcher<'a> {
    sub: CString,
    /// the subscription id.
    pub mid: i32,
    mosq: &'a Mosquitto,
}

impl <'a>TopicMatcher<'a> {
    fn new(sub: CString, mid: i32, mosq: &'a Mosquitto) -> TopicMatcher<'a> {
        TopicMatcher{sub: sub, mid: mid, mosq: mosq}
    }

    /// true if a message matches a subscription topic
    pub fn matches(&self, msg: &MosqMessage) -> bool {
        let mut matched: u8 = 0;
        unsafe {
             mosquitto_topic_matches_sub(self.sub.as_ptr(),msg.msg_ref().topic, &mut matched);
        }
        if matched > 0 {true} else {false}
    }

    fn receive(&self, millis: i32, just_one: bool) -> Result<Vec<MosqMessage>> {
        let t = Instant::now();
        let wait = Duration::from_millis(millis as u64);
        let mut mc = self.mosq.callbacks(Vec::new());
        mc.on_message(|data,msg| {
            if self.matches(&msg) {
                data.push(MosqMessage::new(msg.msg,true));
            }
        });

        while t.elapsed() < wait {
            self.mosq.do_loop(millis)?;
            if just_one && mc.data.len() > 0 {
                break;
            }
        }

        if mc.data.len() > 0 { // we got mail!
            // take results out of the sticky grip of mc data
            let mut res = Vec::new();
            ::std::mem::swap(&mut mc.data, &mut res);
            Ok(res)
        } else { // no messages considered an Error...
            Err(Error::new("receive",MOSQ_ERR_TIMEOUT))
        }

    }

    /// receive and return messages matching this topic, until timeout
    pub fn receive_many(&self, millis: i32) -> Result<Vec<MosqMessage>> {
        self.receive(millis,false)
    }


    /// receive and return exactly one message matching this topic
    pub fn receive_one(&self, millis: i32) -> Result<MosqMessage> {
        self.receive(millis,true).map(|mut v| v.remove(0))
    }
}

/// Mosquitto version
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub revision: u32
}

impl Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f,"{}.{}.{}",self.minor,self.major,self.revision)
    }
}

/// get version of the mosquitto client
pub fn version() -> Version {
    let mut major: c_int = 0;
    let mut minor: c_int = 0;
    let mut revision: c_int = 0;

    unsafe { mosquitto_lib_version(&mut major,&mut minor,&mut revision); }

    Version{major: major as u32,minor: minor as u32,revision: revision as u32}
}

/// Mosquitto client
pub struct Mosquitto {
    mosq: *const Mosq,
    owned: bool,
}

impl Mosquitto {

    /// create a new mosquitto instance, providing a client name.
    /// Clients connecting to a broker must have unique names
    pub fn new(id: &str) -> Mosquitto {
        if INSTANCES.fetch_add(1, Ordering::SeqCst) == 0 {
            // println!("initializing mosq");
            unsafe { mosquitto_lib_init(); }
        }
        let mosq = unsafe { mosquitto_new(cs(id).as_ptr(),1,null()) };
        Mosquitto{
            mosq: mosq,
            owned: true
        }
    }

    /// create a Callback object so you can listen to events.
    pub fn callbacks<'a,T>(&'a self, data: T) -> Callbacks<'a,T> {
        Callbacks::new(self,data)
    }

    /// connect to the broker.
    /// You can only be fully sure that a connection succeeds
    /// after the [on_connect](struct.Callbacks#method.on_connect) callback returns non-zero
    pub fn connect(&self, host: &str, port: u32) -> Result<()> {
        Error::result("connect",unsafe {
             mosquitto_connect(self.mosq,cs(host).as_ptr(),port as c_int,0)
        })
    }

    /// connect to the broker, waiting for success.
    pub fn connect_wait(&self, host: &str, port: u32, millis: i32) -> Result<()> {
        self.connect(host,port)?;
        let t = Instant::now();
        let wait = Duration::from_millis(millis as u64);
        let mut callback = self.callbacks(MOSQ_CONNECT_ERR_TIMEOUT);
        callback.on_connect(|data, rc| {
            *data = rc;
        });
        loop {
            self.do_loop(millis)?;
            if callback.data == MOSQ_CONNECT_ERR_OK {
                return Ok(())
            };
            if t.elapsed() > wait {
                break;
            }
        }
        Err(Error::new_connect(callback.data))

    }

    /// call if you wish to use Mosquitto in a multithreaded environment.
    pub fn threaded(&self) {
        unsafe { mosquitto_threaded_set(self.mosq,1); }
    }

    pub fn reconnect_delay_set(&self,delay: u32, delay_max: u32, exponential_backoff: bool) -> Result<()> {
        Error::result("delay_set",unsafe {
            mosquitto_reconnect_delay_set(self.mosq,
                delay as c_int,
                delay_max as c_int,
                exponential_backoff as u8
        )})

    }

    /// subscribe to an MQTT topic, with a desired quality-of-service.
    /// The returned value can be used to directly match
    /// against received messages, and has a `mid` field identifying
    /// the subscribing request. on_subscribe will be called with this
    /// identifier.
    pub fn subscribe<'a>(&'a self, sub: &str, qos: u32) -> Result<TopicMatcher<'a>> {
        let mut mid: c_int = 0;
        let sub = cs(sub);
        let rc = unsafe { mosquitto_subscribe(self.mosq,&mut mid,sub.as_ptr(),qos as c_int) };
        if rc == 0 {
            Ok(TopicMatcher::new(sub,mid,self))
        } else {
            Err(Error::new("subscribe",rc))
        }
    }

    /// unsubcribe from an MQTT topic - `on_unsubscribe` callback will be called.
    pub fn unsubscribe(&self, sub: &str) -> Result<i32> {
        let mut mid = 0;
        let rc = unsafe { mosquitto_unsubscribe(self.mosq,&mut mid, cs(sub).as_ptr()) };
        if rc == 0 {
            Ok(mid as i32)
        } else {
            Err(Error::new("unsubscribe",rc))
        }
    }

    /// publish an MQTT message to the broker, returning message id.
    /// Quality-of-service and whether retained can be specified.
    /// To be sure, check the message id passed to the `on_publish` callback
    pub fn publish(&self, topic: &str, payload: &[u8], qos: u32, retain: bool) -> Result<i32> {
        let mut mid = 0;

        let rc = unsafe { mosquitto_publish(
            self.mosq,&mut mid, cs(topic).as_ptr(),
            payload.len() as c_int,payload.as_ptr(),
            qos as c_int, if retain {1} else {0}
        )};

        if rc == 0 {
            Ok(mid as i32)
        } else {
            Err(Error::new("publish",rc))
        }
    }

    /// publish an MQTT message to the broker, returning message id after waiting for successful publish
    pub fn publish_wait(&self, topic: &str, payload: &[u8], qos: u32, retain: bool, millis: i32) -> Result<i32> {
        let our_mid = self.publish(topic,payload,qos,retain)?;
        let t = Instant::now();
        let wait = Duration::from_millis(millis as u64);
        let mut callback = self.callbacks(0);
        callback.on_publish(|data, mid| {
            *data = mid;
        });
        loop {
            self.do_loop(millis)?;
            if callback.data == our_mid {
                return Ok(our_mid)
            };
            if t.elapsed() > wait {
                break;
            }
        }
        Err(Error::new("publish",MOSQ_ERR_UNKNOWN))
    }


    /// explicitly disconnect from the broker.
    pub fn disconnect(&self) -> Result<()> {
        Error::result("disconnect",unsafe {
            mosquitto_disconnect(self.mosq)
        })
    }

    /// process network events for at most `timeout` milliseconds.
    /// -1 will mean the default, 1000ms.
    pub fn do_loop(&self, timeout: i32) -> Result<()> {
        Error::result("do_loop",unsafe {
            mosquitto_loop(self.mosq,timeout as c_int,1)
        })
    }

    /// process network events.
    /// This will handle intermittent disconnects for you,
    /// but will return after an explicit [disconnect()](#method.disconnect) call
    pub fn loop_forever(&self, timeout: i32) -> Result<()> {
        Error::result("loop_forever",unsafe {
            mosquitto_loop_forever(self.mosq,timeout as c_int,1)
        })
    }

    /// loop forever, but do not regard an explicit disconnect as an error.
    pub fn loop_until_disconnect(&self, timeout: i32) -> Result<()> {
       if let Err(e) = self.loop_forever(timeout) {
            if e.error() == sys::MOSQ_ERR_NO_CONN {
                Ok(())
            } else { // errror handling......!
                Err(e)
            }
        } else {
            Ok(())
        }
    }

}

// mosquitto is thread-safe, so let's tell Rust about it
unsafe impl Send for Mosquitto {}
unsafe impl Sync for Mosquitto {}

// important that clones do not own the underlying pointer
// and try to free it!
impl Clone for Mosquitto {
    fn clone(&self) -> Mosquitto {
        Mosquitto{
            mosq: self.mosq,
            owned: false
        }
    }
}

impl Drop for Mosquitto {
    fn drop(&mut self) {
        // eprintln!("Mosquitto drop {}",self.owned);
        if self.owned {
            unsafe { mosquitto_destroy(self.mosq); }
            // the last person to leave the building must turn off the lights
            if INSTANCES.fetch_sub(1, Ordering::SeqCst) == 1 {
                // eprintln!("clean up mosq");
                unsafe {mosquitto_lib_init();}
            }
        }
    }
}

/// Handling mosquitto callbacks.
/// This will pass a mutable reference to the
/// contained data to the callbacks.
pub struct Callbacks<'a,T> {
    message_callback: Option<Box<Fn(&mut T,MosqMessage) + 'a>>,
    connect_callback: Option<Box<Fn(&mut T,i32) + 'a>>,
    publish_callback: Option<Box<Fn(&mut T,i32) + 'a>>,
    subscribe_callback: Option<Box<Fn(&mut T,i32) + 'a>>,
    unsubscribe_callback: Option<Box<Fn(&mut T,i32) + 'a>>,
    disconnect_callback: Option<Box<Fn(&mut T,i32) + 'a>>,
    log_callback: Option<Box<Fn(&mut T,u32,&str) + 'a>>,
    mosq: &'a Mosquitto,
    init: bool,
    pub data: T,
}

impl <'a,T> Callbacks<'a,T> {

    /// create a new callback handler with data.
    /// Initialize with an existing Mosquitto reference.
    pub fn new(mosq: &Mosquitto, data: T) -> Callbacks<T> {
        Callbacks {
            message_callback: None,
            connect_callback: None,
            publish_callback: None,
            subscribe_callback: None,
            unsubscribe_callback: None,
            disconnect_callback: None,
            log_callback: None,
            mosq: mosq,
            init: false,
            data: data
        }
    }

    /// a reference to the Mosquitto instance
    pub fn mosq(&self) -> &Mosquitto {
        self.mosq
    }

    fn initialize(&mut self) {
        if ! self.init {
            self.init = true;
            let pdata: *const Callbacks<T> = &*self;
            unsafe {
                mosquitto_user_data_set(self.mosq.mosq, pdata as *const Data);
            };
        }
    }

    /// provide a closure which will be called when messages arrive.
    /// You are passed a mutable reference to data and the message
    pub fn on_message<C: Fn(&mut T,MosqMessage) + 'a>(&mut self, callback: C) {
        self.initialize();
        unsafe {mosquitto_message_callback_set(self.mosq.mosq,mosq_message_callback::<T>);}
        self.message_callback = Some(Box::new(callback));
    }

    /// provide a closure which is called when connection happens.
    /// You are passed a mutable reference to data and the status.
    pub fn on_connect<C: Fn(&mut T,i32) + 'a>(&mut self, callback: C) {
        self.initialize();
        unsafe {mosquitto_connect_callback_set(self.mosq.mosq,mosq_connect_callback::<T>);}
        self.connect_callback = Some(Box::new(callback));
    }

    /// provide a closure which is called after publishing a message.
    /// You are passed a mutable reference to data and the message id.
    pub fn on_publish<C: Fn(&mut T,i32) + 'a>(&mut self, callback: C) {
        self.initialize();
        unsafe {mosquitto_publish_callback_set(self.mosq.mosq,mosq_publish_callback::<T>);}
        self.publish_callback = Some(Box::new(callback));
    }

    /// provide a closure which is called after subscribing.
    /// You are passed a mutable reference to data and the subscription id.
    pub fn on_subscribe<C: Fn(&mut T,i32) + 'a>(&mut self, callback: C) {
        self.initialize();
        unsafe {mosquitto_subscribe_callback_set(self.mosq.mosq,mosq_subscribe_callback::<T>);}
        self.subscribe_callback = Some(Box::new(callback));
    }

    /// provide a closure which is called after unsubscribing from a topic
    /// You are passed a mutable reference to data and the subscription id.
    pub fn on_unsubscribe<C: Fn(&mut T,i32) + 'a>(&mut self, callback: C) {
        self.initialize();
        unsafe {mosquitto_unsubscribe_callback_set(self.mosq.mosq,mosq_unsubscribe_callback::<T>);}
        self.unsubscribe_callback = Some(Box::new(callback));
    }

    /// provide a closure which is called when client disconnects from broker.
    /// You are passed a mutable reference to data and ....
    pub fn on_disconnect<C: Fn(&mut T,i32) + 'a>(&mut self, callback: C) {
        self.initialize();
        unsafe {mosquitto_disconnect_callback_set(self.mosq.mosq,mosq_disconnect_callback::<T>);}
        self.disconnect_callback = Some(Box::new(callback));
    }

    /// provide a closure which is called for each log message
    /// You are passed a mutable reference to data, a logging level,
    /// and the text of the log message
    pub fn on_log<C: Fn(&mut T,u32,&str) + 'a>(&mut self, callback: C) {
        self.initialize();
        unsafe {mosquitto_log_callback_set(self.mosq.mosq,mosq_log_callback::<T>);}
        self.log_callback = Some(Box::new(callback));
    }

}

impl <'a,T>Drop for Callbacks<'a,T> {
    fn drop(&mut self) {
        unsafe {
            mosquitto_user_data_set(self.mosq.mosq, null() as *const Data);
        }
    }
}


// clean up with a macro (suprisingly hard to write as a function)
macro_rules! callback_ref {
    ($data:expr,$T:ident) =>
    {
        unsafe {&mut *($data as *mut Callbacks<$T>)}
    }
}

extern fn mosq_connect_callback<T>(_: *const Mosq, data: *mut Data, rc: c_int) {
    if data.is_null() { return; }
    let this = callback_ref!(data,T);
    if let Some(ref callback) = this.connect_callback {
        callback(&mut this.data, rc as i32);
    }
}

extern fn mosq_publish_callback<T>(_: *const Mosq, data: *mut Data, rc: c_int) {
    if data.is_null() { return; }
    let this = callback_ref!(data,T);
    if let Some(ref callback) = this.publish_callback {
        callback(&mut this.data, rc as i32);
    }
}

extern fn mosq_message_callback<T>(_: *const Mosq, data: *mut Data, message: *const Message) {
    if data.is_null() { return; }
    let this = callback_ref!(data,T);
    //println!("msg {:?}", unsafe {&*message});
    if let Some(ref callback) = this.message_callback {
        callback(&mut this.data, MosqMessage::new(message,false));
    }
}

extern fn mosq_subscribe_callback<T>(_: *const Mosq, data: *mut Data, rc: c_int) {
    if data.is_null() { return; }
    let this = callback_ref!(data,T);
    if let Some(ref callback) = this.subscribe_callback {
        callback(&mut this.data, rc as i32);
    }
}

extern fn mosq_unsubscribe_callback<T>(_: *const Mosq, data: *mut Data, rc: c_int) {
    if data.is_null() { return; }
    let this = callback_ref!(data,T);
    if let Some(ref callback) = this.unsubscribe_callback {
        callback(&mut this.data, rc as i32);
    }
}

extern fn mosq_disconnect_callback<T>(_: *const Mosq, data: *mut Data, rc: c_int) {
    if data.is_null() { return; }
    let this = callback_ref!(data,T);
    if let Some(ref callback) = this.disconnect_callback {
        callback(&mut this.data, rc as i32);
    }
}

extern fn mosq_log_callback<T>(_: *const Mosq, data: *mut Data, level: c_int, text: *const c_char) {
    if data.is_null() { return; }
    let this = callback_ref!(data,T);
    let text = unsafe { CStr::from_ptr(text).to_str().expect("log text was not UTF-8")  };
    if let Some(ref callback) = this.log_callback {
        callback(&mut this.data, level as u32, text);
    }
}

