use crate::{parser::Torrent, tracker::PeerDiscovery};

pub struct Downloader {
    discoverer: PeerDiscovery,
    pub file_buffer: Vec<u8>,
    torrent: Torrent,
}

impl Downloader {
    pub fn new(discoverer: PeerDiscovery, torrent: Torrent) -> Self {
        Self {
            discoverer: discoverer.clone(),
            file_buffer: Vec::new(),
            torrent,
        }
    }

    pub fn download(&mut self) {
        let peers = self.discoverer.discover().unwrap();
        let peer = peers.peers.first().unwrap();
        print!("{}\t", peer.sock_ip);

        let total_pieces = self.torrent.info.pieces.len() / 20;
        for pieces_index in 0..total_pieces {
            let piece = peer.get_piece(&self.torrent.info, pieces_index).unwrap();
            self.file_buffer.extend(piece);
        }
    }
}
