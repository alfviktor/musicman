use async_channel::{Receiver, Sender};
use serde_json::Value;
use std::{
    ffi::{CStr, CString, c_char},
    sync::OnceLock,
};
unsafe extern "C" {
    fn pw_connect(cb: extern "C" fn(*const c_char));
    fn pw_send(method: *const c_char, text: *const c_char);
    fn pw_shutdown();
    fn pw_pump();
}
static EVENTS: OnceLock<Sender<Value>> = OnceLock::new();
extern "C" fn receive(raw: *const c_char) {
    if raw.is_null() {
        return;
    }
    // The adapter's UTF-8 string is valid for this callback only. Deserialize before returning.
    if let Ok(value) = serde_json::from_slice(unsafe { CStr::from_ptr(raw) }.to_bytes())
        && let Some(tx) = EVENTS.get()
    {
        let _ = tx.try_send(value);
    }
}
pub fn connect() -> Receiver<Value> {
    let (tx, rx) = async_channel::unbounded();
    EVENTS.set(tx).expect("one audio engine");
    unsafe { pw_connect(receive) };
    rx
}
pub fn send(method: &str, data: Value) {
    let key = CString::new(method).unwrap();
    let bytes = CString::new(data.to_string()).unwrap();
    unsafe { pw_send(key.as_ptr(), bytes.as_ptr()) }
}
pub fn shutdown() {
    unsafe { pw_shutdown() }
}
pub fn pump() {
    unsafe { pw_pump() }
}
