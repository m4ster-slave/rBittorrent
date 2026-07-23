//! # rBittorrent
//! Simple implementation of the BitTorrent protocoll in rust with minimal dependencies

use std::{fs::File, io::Write};

use anyhow::Ok;
use tokio::task::JoinSet;

use crate::{discovery::PeerDiscoverer, downloader::Downloader};

mod discovery;
mod downloader;
mod parser;
mod peer_connection;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Error: Specify at least one torrent file");
        println!("Usage: {} /path/to/file.torrent", args[0]);
        return;
    }

    let mut task_handle = JoinSet::new();

    // download each torrent file
    for file in &args[1..args.len()] {
        let copy = file.clone();
        task_handle.spawn(async move {
            let torrent = parser::parse_torrent_file(copy.clone())?;
            let file_name = torrent.info.name.clone();

            println!("Downloading {}:\n{}", copy, torrent);

            let discoverer = PeerDiscoverer::new("rBittorrent", 6969, torrent.clone()).await;
            let mut downloader = Downloader::new(&discoverer, torrent);
            downloader.download().await;

            let mut out_file = File::create_new(file_name)?;
            out_file.write_all(&downloader.file_buffer)?;
            Ok(())
        });
    }

    for res in task_handle.join_all().await {
        if let Err(e) = res {
            eprintln!("Task panicked: {:?}", e);
        }
    }
}
