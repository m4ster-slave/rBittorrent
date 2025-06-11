mod parser;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file = &args[1];
    let file_content = std::fs::read_to_string(file).unwrap();

    println!("Content: {}", parser::parse_value(&file_content).unwrap());
}
