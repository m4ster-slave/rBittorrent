use serde::Deserialize;
use serde_bencode::de;
use std::net::{Ipv4Addr, SocketAddrV4};

use crate::parser::{calculate_urlencoded_info_hash, Torrent};
use crate::peer_connection::Peer;

#[derive(Clone)]
pub struct PeerDiscoverer {
    announce_url: String,
    infohash: String,
    peer_id: Vec<u8>,
    port: u16,
    uploaded: usize,
    pub downloaded: usize,
    pub left: usize,
    compact: usize,
}

#[derive(Debug, Deserialize)]
struct PeerResponseSer {
    interval: usize,
    #[serde(rename(deserialize = "min interval"))]
    min_interval: Option<usize>,
    #[serde(with = "serde_bytes")]
    peers: Vec<u8>, // TODO parse this immediatly i just cant figure out how rn
}

#[derive(Debug)]
pub struct PeerResponse {
    /// The number of seconds the downloader should wait between regular rerequests
    pub interval: usize,
    pub _min_interval: Option<usize>,
    /// list of dictionaries corresponding to peers, each of which contains the keys peer id, ip,
    /// and port, which map to the peer's self-selected ID, IP address or dns name as a string, and
    pub peers: Vec<Peer>,
}

impl PeerDiscoverer {
    pub async fn new(peer_id: &str, port: u16, torrent: Torrent) -> Self {
        let mut peer_id_bytes = peer_id.as_bytes();

        if peer_id_bytes.len() > 20 {
            eprintln!("Peer ID was too long, using default: 'defaultBittorrentclient'");
            peer_id_bytes = b"defaultBittorrentclient";
        }

        let mut padded_peer_id = [0u8; 20];
        let len = peer_id_bytes.len().min(20);
        padded_peer_id[..len].copy_from_slice(&peer_id_bytes[..len]);

        Self {
            announce_url: torrent.announce,
            infohash: calculate_urlencoded_info_hash(&torrent.info).unwrap(),
            peer_id: padded_peer_id.to_vec(),
            port,
            uploaded: 0,
            downloaded: 0,
            left: match torrent.info.file_tree {
                crate::parser::FileTree::SingleFile { length } => length,
                crate::parser::FileTree::MultiFile { files } => {
                    files.iter().map(|file| file.length).sum()
                }
            },
            compact: 1,
        }
    }

    /// function to disvoer your peers, after a new peer is discovered we get its handshake
    pub async fn discover(&mut self, torrent: &Torrent) -> Result<PeerResponse, anyhow::Error> {
        let url = format!(
            "{}/?info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact={}",
            self.announce_url,
            self.infohash,
            String::from_utf8_lossy(&self.peer_id),
            self.port,
            self.uploaded,
            self.downloaded,
            self.left,
            self.compact
        );
        let resp = reqwest::get(url).await?;
        let body = resp.bytes().await?;

        let peer_response_ser: PeerResponseSer = de::from_bytes(&body)?;

        let mut peers: Vec<Peer> = Vec::new();
        for peer in peer_response_ser.peers.chunks(6) {
            let p1 = peer[4] as u16;
            let p2 = peer[5] as u16;
            let port = (p1 << 8) | p2;
            let ip = Ipv4Addr::new(peer[0], peer[1], peer[2], peer[3]);
            let sock_ip = SocketAddrV4::new(ip, port);

            peers.push(Peer {
                sock_ip,
                available: Vec::new(),
                conn: None,
                peer_choking: true,
            });
        }

        for peer in peers.iter_mut() {
            peer.perform_handshake(&torrent.info).await?;
        }

        Ok(PeerResponse {
            interval: peer_response_ser.interval,
            _min_interval: peer_response_ser.min_interval,
            peers,
        })
    }
}
