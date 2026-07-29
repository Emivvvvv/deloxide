use deloxide::{Deloxide, RwLock};

fn main() {
    Deloxide::new().start().expect("detector initialization");
    let value = RwLock::new(String::from("deloxide"));
    assert_eq!(value.read().as_str(), "deloxide");
    value.write().push_str(" runtime");
    assert_eq!(value.read().as_str(), "deloxide runtime");
}
