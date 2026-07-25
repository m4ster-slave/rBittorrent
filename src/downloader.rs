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
        let mut interval = time::interval(Duration::from_secs(1));

        loop {
            tokio::select! {
                _ = interval.tick() => {
                    let discovery = self.discoverer.discover(&self.torrent).await.unwrap();
                    self.peers = discovery.peers;
                    println!("Peers updated: {} peers", self.peers.len());

                    self.peers.iter().for_each(|p| println!("{:?}", p));

                    interval = time::interval(Duration::from_secs(discovery.interval as u64));
                }
                else => break,
            }

            if !self.peers.is_empty() {
                let peer: &mut Peer = self.peers.first_mut().unwrap();
                println!("Downloading from: {}", peer.sock_ip);
                let total_pieces = self.torrent.info.pieces.len() / 20;

                for pieces_index in 0..total_pieces {
                    println!("Getting piece {pieces_index}/{total_pieces}");
                    let piece = peer
                        .get_piece(&self.torrent.info, pieces_index)
                        .await
                        .unwrap_or_else(|e| {
                            println!("Error getting piece from peer {}", e);
                            panic!()
                        });
                    self.file_buffer.extend(piece);
                }

                println!("Download complete");
                break;
            }
        }
    }
}
