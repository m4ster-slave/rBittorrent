use crate::{discovery::PeerDiscoverer, parser::Torrent, peer_connection::Peer};

use tokio::time::{self, Duration};

/// Responsible for downloading the file
pub struct Downloader {
    discoverer: PeerDiscoverer,
    peers: Vec<Peer>,
    pub file_buffer: Vec<u8>,
    torrent: Torrent,
}

impl Downloader {
    pub fn new(discoverer: &PeerDiscoverer, torrent: Torrent) -> Self {
        Self {
            discoverer: discoverer.clone(),
            peers: Vec::new(),
            file_buffer: Vec::new(),
            torrent,
        }
    }

    pub async fn download(&mut self) {
        let init_discovery = self.discoverer.discover().await.unwrap();
        self.peers = init_discovery.peers;

        let mut interval = time::interval(Duration::from_secs(init_discovery.interval as u64));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let discovery = self.discoverer.discover().await.unwrap();
                    self.peers = discovery.peers;
                    println!("Peers updated: {} peers", self.peers.len());
                }
                else => break,
            }

            if !self.peers.is_empty() {
                let peer = self.peers.first().unwrap();
                println!("Downloading from: {}", peer.sock_ip);
                let total_pieces = self.torrent.info.pieces.len() / 20;

                for pieces_index in 0..total_pieces {
                    let piece = peer.get_piece(&self.torrent.info, pieces_index).unwrap();
                    self.file_buffer.extend(piece);
                }

                println!("Download complete");
                break;
            }
        }
    }
}
