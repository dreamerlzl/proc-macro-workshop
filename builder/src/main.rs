use derive_builder::Builder;

#[derive(Builder, Debug)]
pub struct Foo {
    a: i32,
}

fn main() {
    let f = Foo::builder().a(2);
}
