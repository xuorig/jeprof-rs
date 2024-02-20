use std::fs;

use jeprof_rs::Profile;

fn main() {
    let file_path = std::env::args().nth(1).expect("no file given");
    let profile = fs::read_to_string(file_path).unwrap();
    let profile = Profile::parse(&profile);
    println!("{:#?}", profile);
}
