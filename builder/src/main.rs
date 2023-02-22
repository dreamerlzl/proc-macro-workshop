use derive_builder::Builder;

//#[derive(Builder)]
//struct Command(u32);

//#[derive(Builder)]
//enum Command {
//    Foo,
//    Bar,
//}

#[derive(Builder)]
pub struct Command {
    executable: String,
    #[builder(each = "arg")]
    args: Vec<String>,
    env: Vec<String>,
    current_dir: Option<String>,
}

fn main() {}
