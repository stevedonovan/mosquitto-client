use std::os::raw::{c_int,c_char};
use std::ffi::CStr;


pub const MOSQ_ERR_CONN_PENDING:i32 = -1;
pub const MOSQ_ERR_SUCCESS:i32 = 0;
pub const MOSQ_ERR_NOMEM:i32 = 1;
pub const MOSQ_ERR_PROTOCOL:i32 = 2;
pub const MOSQ_ERR_INVAL:i32 = 3;
pub const MOSQ_ERR_NO_CONN:i32 = 4;
pub const MOSQ_ERR_CONN_REFUSED:i32 = 5;
pub const MOSQ_ERR_NOT_FOUND:i32 = 6;
pub const MOSQ_ERR_CONN_LOST:i32 = 7;
pub const MOSQ_ERR_TLS:i32 = 8;
pub const MOSQ_ERR_PAYLOAD_SIZE:i32 = 9;
pub const MOSQ_ERR_NOT_SUPPORTED:i32 = 10;
pub const MOSQ_ERR_AUTH:i32 = 11;
pub const MOSQ_ERR_ACL_DENIED:i32 = 12;
pub const MOSQ_ERR_UNKNOWN:i32 = 13;
pub const MOSQ_ERR_ERRNO:i32 = 14;
pub const MOSQ_ERR_EAI:i32 = 15;
pub const MOSQ_ERR_PROXY:i32 = 16;

pub const MOSQ_LOG_NONE:i32 = 0x00;
pub const MOSQ_LOG_INFO:i32 = 0x01;
pub const MOSQ_LOG_NOTICE:i32 = 0x02;
pub const MOSQ_LOG_WARNING:i32 = 0x04;
pub const MOSQ_LOG_ERR:i32 = 0x08;
pub const MOSQ_LOG_DEBUG:i32 = 0x10;
pub const MOSQ_LOG_SUBSCRIBE:i32 = 0x20;
pub const MOSQ_LOG_UNSUBSCRIBE:i32 = 0x40;
pub const MOSQ_LOG_ALL:i32 = 0xFFFF;

pub const MOSQ_CONNECT_ERR_OK:i32 = 0;
pub const MOSQ_CONNECT_ERR_PROTOCOL:i32 = 1;
pub const MOSQ_CONNECT_ERR_BADID:i32 = 2;
pub const MOSQ_CONNECT_ERR_NOBROKER:i32 = 3;
pub const MOSQ_CONNECT_ERR_TIMEOUT:i32 = 256;

pub type Mosq = c_int;
pub type Data = c_int;

#[derive(Debug)]
#[repr(C)]
pub struct Message{
	pub mid: c_int,
	pub topic: *const c_char,
	pub payload: *const u8,
	pub payloadlen: c_int,
	pub qos: c_int,
	pub retain: u8
}

#[link(name = "mosquitto")]
extern {
    pub fn mosquitto_lib_version(major: *mut c_int, minor: *mut c_int, revision: *mut c_int) -> c_int;
    pub fn mosquitto_lib_init() -> c_int;
    pub fn mosquitto_lib_cleanup() -> c_int;
    pub fn mosquitto_new(id: *const c_char, clean_session: u8, obj: *const Data) -> *mut Mosq;
    pub fn mosquitto_destroy(mosq: *const Mosq);
    pub fn mosquitto_connect(mosq: *const Mosq, host: *const c_char, port: c_int, keepalive: c_int) -> c_int;
    pub fn mosquitto_reconnect(mosq: *const Mosq) -> c_int;
    pub fn mosquitto_disconnect(mosq: *const Mosq) -> c_int;
    pub fn mosquitto_strerror(err: c_int) -> *const c_char;
    pub fn mosquitto_user_data_set(mosq: *const Mosq, obj: *const Data);
    pub fn mosquitto_threaded_set(mosq: *const Mosq, threaded: u8) -> c_int;

    pub fn mosquitto_connect_callback_set(mosq: *const Mosq,
        callback: extern fn(*const Mosq, *mut Data, c_int)
    );
    pub fn mosquitto_publish_callback_set(mosq: *const Mosq,
        callback: extern fn(*const Mosq, *mut Data, c_int)
    );
    pub fn mosquitto_subscribe_callback_set(mosq: *const Mosq,
        callback: extern fn(*const Mosq, *mut Data, c_int)
    );

    pub fn mosquitto_message_callback_set(mosq: *const Mosq,
        callback: extern fn(*const Mosq, *mut Data, *const Message)
    );

    pub fn mosquitto_disconnect_callback_set(mosq: *const Mosq,
        callback: extern fn(*const Mosq, *mut Data, c_int)
    );
    pub fn mosquitto_unsubscribe_callback_set(mosq: *const Mosq,
        callback: extern fn(*const Mosq, *mut Data, c_int)
    );

    pub fn mosquitto_log_callback_set(mosq: *const Mosq,
        callback: extern fn(*const Mosq, *mut Data, c_int, *const c_char)
    );

    pub fn mosquitto_message_copy(copy: *mut Message, msg: *const Message) -> c_int;
    pub fn mosquitto_message_free(msg: *const *const Message);

    pub fn mosquitto_subscribe(mosq: *const Mosq, mid: *mut c_int, sub: *const c_char, qos: c_int) -> c_int;
    pub fn mosquitto_unsubscribe(mosq: *const Mosq,mid: *mut c_int, sub: *const c_char) -> c_int;

    pub fn mosquitto_publish(mosq: *const Mosq, mid: *mut c_int, topic: *const c_char,
        payloadlen: c_int, payload: *const u8, qos: c_int, retain: u8) -> c_int;

    pub fn mosquitto_loop(mosq: *const Mosq, timeout: c_int, max_packets: c_int) -> c_int;
    pub fn mosquitto_loop_forever(mosq: *const Mosq, timeout: c_int, max_packets: c_int) -> c_int;

    pub fn mosquitto_topic_matches_sub(sub: *const c_char, topic: *const c_char, result: *mut u8);

}

pub fn mosq_strerror(rc: c_int) -> String {
    unsafe {
        let errs = mosquitto_strerror(rc);
        CStr::from_ptr(errs).to_str().unwrap().to_string()
    }
}

