use serde_bencode::de;

use crate::parser::{calculate_urlencoded_info_hash, AnnounceUrl, Torrent};
use crate::tracker_response::TrackerResponse;

#[derive(Clone)]
pub struct PeerDiscoverer {
    announce_url: AnnounceUrl,
    infohash: String,
    peer_id: Vec<u8>,
    port: u16,
    uploaded: usize,
    pub downloaded: usize,
    pub left: usize,
    compact: usize,
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

        let mut announce_url = torrent.announce.clone();
        if !matches!(torrent.announce, AnnounceUrl::Http(_) | AnnounceUrl::Udp(_)) {
            let list = torrent.announce_list.ok_or_else(|| {
                println!("No announce list and the standard announce url is not udp/http");
                panic!();
            });

            // 2. Flatten Vec<Vec<AnnounceUrl>> into an iterator of &AnnounceUrl
            //    and find the first variant matching Http or Udp
            announce_url = list
                .iter()
                // this is the ugliest code that has ever been written in the history of mankind and
                // i apoligize in advance for anyone who ever has to lay their eyes on this
                .flatten()
                .flatten()
                .find(|url| matches!(url, AnnounceUrl::Http(_) | AnnounceUrl::Udp(_)))
                .ok_or_else(|| {
                    println!("No HTTP or UDP announce URL found in the list");
                    todo!("Any url type but HTTP or UDP hasnt been implemented yet")
                })
                .unwrap()
                .clone();
        }

        Self {
            announce_url,
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
    pub async fn discover(&mut self, torrent: &Torrent) -> Result<TrackerResponse, anyhow::Error> {
        let mut response: TrackerResponse = match &self.announce_url {
            AnnounceUrl::Http(announce_url) => {
                let url = format!(
                    "{}/?info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact={}",
                    announce_url,
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

                de::from_bytes(&body)?
            }
            _ => {
                todo!()
            }
        };

        for peer in response.peers.iter_mut() {
            peer.perform_handshake(&torrent.info).await?;
        }

        Ok(response)
    }
}
