use std::{thread, time};

use crate::tracker::PeerDiscovery;

mod parser;
mod peer_connection;
mod tracker;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file = &args[1];
    let torrent_file = parser::parse_torrent_file(file).unwrap();

    // parse the .torrent file
    println!("{}:\n{}", file, torrent_file);

    let mut discoverer = PeerDiscovery::new("Lukiana", 6969, torrent_file.clone());
    loop {
        println!("Discovery requests: ");
        let peers = discoverer.discover().unwrap();
        for peer in peers.peers {
            print!("{}\t", peer.sock_ip);
            println!(
                "handshake: {}",
                peer.send_handshake(&torrent_file.info).unwrap()
            );
        }
        println!(
            "Waiting for specified interval by the Tracker({}s)",
            peers.interval
        );
        thread::sleep(time::Duration::from_secs(peers.interval as u64));
    }
}
