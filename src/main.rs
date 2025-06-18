use std::{fs::File, io::Write};

use crate::{downloader::Downloader, tracker::PeerDiscovery};

mod downloader;
mod parser;
mod peer_connection;
mod tracker;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file = &args[1];
    let torrent = parser::parse_torrent_file(file).unwrap();
    let file_name = torrent.info.name.clone();

    // parse the .torrent file
    println!("{}:\n{}", file, torrent);

    let discoverer = PeerDiscovery::new("Lukiana", 6969, torrent.clone());
    let mut downloader = Downloader::new(discoverer, torrent);
    downloader.download();

    let mut out_file = File::create_new(file_name).unwrap();
    out_file.write_all(&downloader.file_buffer).unwrap();
}
