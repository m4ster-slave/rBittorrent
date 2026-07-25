use std::net::SocketAddr;

use serde_bencode::de;
use tokio::net::UdpSocket;
use url::form_urlencoded;

use crate::parser::{calculate_info_hash_bytes, AnnounceUrl, Torrent};
use crate::peer_connection::Peer;
use crate::tracker_response::TrackerResponse;
use crate::udp_tracker::{
    Action, AnnounceRequest, AnnounceResponse, ConnectRequest, ConnectResponse, TrackerError,
};

#[derive(Clone)]
pub struct PeerDiscoverer {
    announce_url: AnnounceUrl,
    infohash: [u8; 20],
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
            infohash: calculate_info_hash_bytes(&torrent.info).unwrap(),
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
                                    form_urlencoded::byte_serialize(&self.infohash).collect::<String>(),
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

            AnnounceUrl::Udp(announce_url) => {
                let parsed_url = url::Url::parse(announce_url)?;
                let host = parsed_url.host_str().ok_or_else(|| {
                    anyhow::anyhow!("UDP announce URL has no host: {}", announce_url)
                })?;
                let port = parsed_url.port().ok_or_else(|| {
                    anyhow::anyhow!("UDP announce URL has no port: {}", announce_url)
                })?;

                // host may be a domain name, so this needs an actual DNS lookup rather than
                // a naive SocketAddr::parse on the raw string
                let remote_addr: SocketAddr =
                    tokio::net::lookup_host((host, port))
                        .await?
                        .next()
                        .ok_or_else(|| anyhow::anyhow!("Could not resolve host: {}", host))?;

                let local_addr: SocketAddr = if remote_addr.is_ipv4() {
                    "0.0.0.0:0"
                } else {
                    unimplemented!();
                }
                .parse()?;

                let socket = UdpSocket::bind(local_addr).await?;
                socket.connect(&remote_addr).await?;

                // 1. Send Connect Request
                let my_transaction_id = rand::random::<i32>();
                let connect_request = ConnectRequest::new(my_transaction_id);
                socket.send(&connect_request.serialize()).await?;

                // 2. Receive Connect Response
                let mut data = vec![0u8; 2048]; // 2KB buffer is plenty
                let len = socket.recv(&mut data).await?;
                let response_bytes = &data[..len];

                // Check for tracker errors on connect
                if len >= 4 {
                    let action_id = i32::from_be_bytes(response_bytes[0..4].try_into()?);
                    if action_id == Action::Error as i32 {
                        let err = TrackerError::parse(response_bytes)?;
                        anyhow::bail!("Tracker returned error on connect: {}", err.error_string);
                    }
                }

                // Parse standard connect response
                let connect_response = ConnectResponse::parse(response_bytes)?;
                println!(
                    "Successfully connected! Connection ID: {}",
                    connect_response.connection_id
                );

                // 3. Send Announce Request
                let announce_transaction_id = rand::random::<i32>();

                // info_hash_bytes.copy_from_slice(&raw_20_byte_hash_here);

                let mut peer_id_bytes = [0u8; 20];
                peer_id_bytes.copy_from_slice(&self.peer_id);

                let announce_request = AnnounceRequest::new(
                    connect_response.connection_id,
                    announce_transaction_id,
                    self.infohash,
                    peer_id_bytes,
                    self.downloaded as i64,
                    self.left as i64,
                    self.uploaded as i64,
                    self.port,
                );

                socket.send(&announce_request.serialize()).await?;

                // 4. Receive Announce Response
                let len = socket.recv(&mut data).await?;
                let announce_resp_bytes = &data[..len];

                // Check for tracker errors on announce
                if len >= 4 {
                    let action_id = i32::from_be_bytes(announce_resp_bytes[0..4].try_into()?);
                    if action_id == Action::Error as i32 {
                        let err = TrackerError::parse(announce_resp_bytes)?;
                        anyhow::bail!("Tracker returned error on announce: {}", err.error_string);
                    }
                }

                let announce_response = AnnounceResponse::parse(announce_resp_bytes)?;

                TrackerResponse {
                    interval: announce_response.interval,
                    peers: announce_response
                        .peers
                        .iter()
                        .map(|p| Peer {
                            sock_ip: p.clone(),
                            available: Vec::new(),
                            conn: None,
                            peer_choking: true,
                        })
                        .collect(),
                }
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
