//! # rBittorrent
//! Simple implementation of the BitTorrent protocoll in rust with minimal dependencies

use std::{fs::File, io::Write};

use crate::{discovery::PeerDiscoverer, downloader::Downloader};

mod discovery;
mod downloader;
mod parser;
mod peer_connection;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Error: Specify at least one torrent file");
        println!("Usage: {} /path/to/file.torrent", args[0]);
        return;
    }

    // download each torrent file
    for file in &args[1..args.len()] {
        let torrent = parser::parse_torrent_file(file).unwrap();
        let file_name = torrent.info.name.clone();

        println!("Downloading {}:\n{}", file, torrent);

        let discoverer = PeerDiscoverer::new("rBittorrent", 6969, torrent.clone());
        let mut downloader = Downloader::new(&discoverer, torrent);
        downloader.download();

        let mut out_file = File::create_new(file_name).unwrap();
        out_file.write_all(&downloader.file_buffer).unwrap();
    }
}
