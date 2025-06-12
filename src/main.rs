mod parser;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file = &args[1];
    let torrent_file = parser::parse_torrent_file(file).unwrap();

    // parse the .torrent file
    println!("{}:\n{}", file, torrent_file);
}
