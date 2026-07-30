//! # rBittorrent
//! Simple implementation of the BitTorrent protocoll in rust with minimal dependencies

use std::{fs::File, io::Write, path::PathBuf};

use anyhow::Ok;
use tokio::task::JoinSet;

use crate::{discovery::PeerDiscoverer, downloader::Downloader, parser::FileTree};

mod discovery;
mod downloader;
mod parser;
mod peer_connection;
mod tracker_response;
mod udp_tracker;

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        println!("Error: Specify at least one torrent file");
        println!("Usage: {} /path/to/file.torrent", args[0]);
        return;
    }

    // TODO validate the files and accept magnet links
    // We have to potentially make announce an array in the Torrent struct...
    // magnet link is censored idk if github scans for maliscous file content liek that
    // magnet:?xt=urn:btih:1A8CXXXXXXXXXXXX4D2DDC3401C0DCF52C9CF9F&dn=XXXXXXXXXXXXXXXXXX&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce&tr=udp%3A%2F%2Fopen.demonii.com%3A1337%2Fannounce&tr=http%3A%2F%2Fopen.tracker.cl%3A1337%2Fannounce&tr=udp%3A%2F%2Fopen.stealth.si%3A80%2Fannounce&tr=udp%3A%2F%2Ftracker.torrent.eu.org%3A451%2Fannounce&tr=udp%3A%2F%2Fexplodie.org%3A6969%2Fannounce&tr=udp%3A%2F%2Fexodus.desync.com%3A6969%2Fannounce&tr=udp%3A%2F%2Ftracker.ololosh.space%3A6969%2Fannounce&tr=udp%3A%2F%2Ftracker.dump.cl%3A6969%2Fannounce&tr=udp%3A%2F%2Ftracker.bittor.pw%3A1337%2Fannounce&tr=udp%3A%2F%2Ftracker-udp.gbitt.info%3A80%2Fannounce&tr=udp%3A%2F%2Fretracker01-msk-virt.corbina.net%3A80%2Fannounce&tr=udp%3A%2F%2Fopen.free-tracker.ga%3A6969%2Fannounce&tr=udp%3A%2F%2Fns-1.x-fins.com%3A6969%2Fannounce&tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce&tr=http%3A%2F%2Ftracker.openbittorrent.com%3A80%2Fannounce&tr=udp%3A%2F%2Fopentracker.i2p.rocks%3A6969%2Fannounce&tr=udp%3A%2F%2Ftracker.internetwarriors.net%3A1337%2Fannounce&tr=udp%3A%2F%2Ftracker.leechers-paradise.org%3A6969%2Fannounce&tr=udp%3A%2F%2Fcoppersurfer.tk%3A6969%2Fannounce&tr=udp%3A%2F%2Ftracker.zer0day.to%3A1337%2Fannounce

    let mut task_handle = JoinSet::new();

    // download each torrent file
    for file in &args[1..args.len()] {
        let copy = file.clone();
        task_handle.spawn(async move {
            let torrent = parser::parse_torrent_file(copy.clone()).unwrap_or_else(|e| {
                println!("Parser error: {e}");
                panic!()
            });
            let file_name = torrent.info.name.clone();

            println!("Downloading {}:\n{}", copy, torrent);

            let discoverer = PeerDiscoverer::new("rBittorrent", 6969, torrent.clone()).await;
            let mut downloader = Downloader::new(&discoverer, &torrent);
            downloader.download().await;

            // writing file to disk
            match &torrent.info.file_tree {
                FileTree::SingleFile { .. } => {
                    let mut out_file = File::create_new(file_name)?;
                    out_file.write_all(&downloader.file_buffer)?;
                }
                FileTree::MultiFile { files } => {
                    let base_dir = PathBuf::from(&file_name);
                    std::fs::create_dir_all(&base_dir)?;

                    let mut buffer_offset = 0;

                    for file_info in files {
                        let mut current_file_path = base_dir.clone();

                        for path_segment in &file_info.path {
                            current_file_path.push(path_segment);
                        }

                        if let Some(parent) = current_file_path.parent() {
                            std::fs::create_dir_all(parent)?;
                        }

                        let mut out_file = File::create(&current_file_path)?;
                        let file_data = &downloader.file_buffer
                            [buffer_offset..buffer_offset + file_info.length];
                        out_file.write_all(file_data)?;

                        buffer_offset += file_info.length;
                    }
                }
            }

            Ok(())
        });
    }

    for res in task_handle.join_all().await {
        if let Err(e) = res {
            eprintln!("Task panicked: {:?}", e);
        }
    }
}
