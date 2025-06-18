use crate::{discovery::PeerDiscoverer, parser::Torrent, peer_connection::Peer};

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

    pub fn download(&mut self) {
        self.peers = self.discoverer.discover().unwrap().peers;
        let peer = self.peers.first().unwrap();
        print!("{}\t", peer.sock_ip);

        let total_pieces = self.torrent.info.pieces.len() / 20;
        for pieces_index in 0..total_pieces {
            let piece = peer.get_piece(&self.torrent.info, pieces_index).unwrap();
            self.file_buffer.extend(piece);
        }
    }
}
