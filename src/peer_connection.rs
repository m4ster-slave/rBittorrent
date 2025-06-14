use std::{
    io::{Read, Write},
    net::{SocketAddrV4, TcpStream},
};

use crate::parser::{calculate_info_hash_bytes, Info};
/// Peer connections are symmetrical. Messages sent in both directions look the same, and data can
/// flow in either direction.
///
/// The peer protocol refers to pieces of the file by index as described in the metainfo file,
/// starting at zero. When a peer finishes downloading a piece and checks that the hash matches, it
/// announces that it has that piece to all of its peers.
///
/// Connections contain two bits of state on either end: *choked or not*, and *interested or not*.
/// Choking is a notification that no data will be sent until unchoking happens. Data transfer
/// takes place whenever one side is interested and the other side is not choking. *Interest state
/// must be kept up to date at all times* - whenever a downloader doesn't have something they
/// currently would ask a peer for in unchoked, they must express lack of interest, despite being
/// choked. Connections start out choked and not interested.
#[derive(Debug)]
pub struct Peer {
    pub sock_ip: SocketAddrV4,
}

// length: u8,
// protocol_string: [char; 19],
// zero_bytes: [u8; 8],
// infohash: [u8; 20],
// peer_id: [u8; 20],
// TODO: Make this a struct and read the direct memory into a buffer
fn generate_handshake(infohash: &[u8]) -> Vec<u8> {
    let mut handshake: Vec<u8> = Vec::new();
    handshake.push(19);
    handshake.extend_from_slice("BitTorrent protocol".as_bytes());
    handshake.extend_from_slice(&[0u8; 8]);
    handshake.extend_from_slice(&infohash[0..20]);
    handshake.extend_from_slice("00112233445566778899".as_bytes());
    handshake
}

struct PeerMessage {
    length: u64,
    message_id: u8,
    payload: Vec<u8>,
}

impl Peer {
    pub fn send_handshake(&self, info_dict: &Info) -> Result<String, Box<dyn std::error::Error>> {
        let infohash = calculate_info_hash_bytes(info_dict)?;
        let handshake = generate_handshake(&infohash);
        let mut stream = TcpStream::connect(self.sock_ip)?;
        let _ = stream.write(&handshake)?;

        let mut buf = vec![0u8; 68];
        stream.read_exact(&mut buf)?;

        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf)?;
        let msg_len = u32::from_be_bytes(len_buf);

        if msg_len == 0 {
            println!("Received keep-alive");
            return Ok(hex::encode(&buf[47..68]));
        }

        let mut msg_buf = vec![0u8; msg_len as usize];
        stream.read_exact(&mut msg_buf)?;

        let message_id = msg_buf[0];
        let payload = msg_buf[1..].to_vec();

        println!("Message ID: {}", message_id);
        println!("Payload: {:?}", payload);

        Ok(hex::encode(&buf[47..68]))
    }
}
