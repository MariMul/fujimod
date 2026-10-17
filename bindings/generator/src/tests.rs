use super::*;
#[test]
fn new_rust_values_reach_every_language_without_copied_bounds() {
    let source = r#"
 pub enum NewChoice {First,Second}
 pub enum NewResult {Done,Unknown(u8)}
 pub struct NewLevel(u8);
 impl NewLevel {
  pub fn new(value:u8)->Result<Self,CommandError>{if value<42 {Ok(Self(value))}else{panic!()}}
  pub fn value(self)->u8 {self.0}
 }
 enum PrivateChoice {Hidden}
 "#;
    let api = Api::from_sources([source]).unwrap();
    let python = api.python();
    let javascript = api.javascript();
    let exports = api.js_exports(false);
    for code in [&python, &javascript, &exports] {
        assert!(code.contains("NewChoice"));
        assert!(code.contains("NewResult"));
        assert!(code.contains("NewLevel"));
        assert!(!code.contains("PrivateChoice"));
        assert!(!code.contains("42"));
    }
    assert!(python.contains("core::NewLevel::new"));
    assert!(javascript.contains("core::NewLevel::new"));
    assert!(!exports.contains("raw_status")); // Python's runtime supplies the common variant field.
    syn::parse_file(&python).unwrap();
    syn::parse_file(&javascript).unwrap();
    assert_eq!(api.service.quantities[0].input, "u8");
}
#[test]
fn source_order_does_not_change_generated_files() {
    let first = "pub enum Zebra {One,Two}";
    let second = "pub enum Alpha {Three,Four}";
    let a = Api::from_sources([first, second]).unwrap();
    let b = Api::from_sources([second, first]).unwrap();
    assert_eq!(a.python(), b.python());
    assert_eq!(a.javascript(), b.javascript());
    assert_eq!(a.js_exports(false), b.js_exports(false));
}
#[test]
fn checked_values_require_a_public_observer_and_byte_input_is_distinct() {
    assert!(
        Api::from_sources([
            "pub struct Level(u8);impl Level {pub fn new(value:u8)->Self {Self(value)}}"
        ])
        .unwrap_err()
        .contains("getter")
    );
    let api=Api::from_sources(["pub struct Packet([u8;6]);impl Packet {pub fn new(data:&[u8])->Result<Self,Error>{todo!()} pub fn data(&self)->&[u8;6]{&self.0}}"]).unwrap();
    assert_eq!(api.service.quantities[0].input, "bytes");
    assert!(api.python().contains("PyBytes::new"));
    assert!(api.javascript().contains("error::buffer"));
}
