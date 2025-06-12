use std::net::Ipv4Addr;

use serde::Deserialize;
use serde_bencode::de;

use crate::parser::{calculate_urlencoded_info_hash, Torrent};

pub struct PeerDiscovery {
    announce_url: String,
    infohash: String,
    peer_id: String,
    port: u16,
    uploaded: usize,
    downloaded: usize,
    left: usize,
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

#[derive(Debug)]
pub struct Peer {
    pub port: u16,
    pub ip: Ipv4Addr,
}

impl PeerDiscovery {
    pub fn new(_peer_id: &str, port: u16, torrent: Torrent) -> Self {
        Self {
            announce_url: torrent.announce,
            infohash: calculate_urlencoded_info_hash(&torrent.info).unwrap(),
            // TODO be able to choose peer id
            peer_id: "00112233445566778899".to_string(),
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

    pub fn discover(&mut self) -> Result<PeerResponse, Box<dyn std::error::Error>> {
        let url = format!(
            "{}/?info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact={}",
            self.announce_url,
            self.infohash,
            self.peer_id,
            self.port,
            self.uploaded,
            self.downloaded,
            self.left,
            self.compact
        );
        let resp = reqwest::blocking::get(url)?;
        let body = resp.bytes()?;

        let peer_response_ser: PeerResponseSer = de::from_bytes(&body)?;

        let mut peers: Vec<Peer> = Vec::new();
        for peer in peer_response_ser.peers.chunks(6) {
            let p1 = peer[4] as u16;
            let p2 = peer[5] as u16;
            let port = (p1 << 8) | p2;
            let ip = Ipv4Addr::new(peer[0], peer[1], peer[2], peer[3]);

            peers.push(Peer { port, ip });
        }

        Ok(PeerResponse {
            interval: peer_response_ser.interval,
            _min_interval: peer_response_ser.min_interval,
            peers,
        })
    }
}
