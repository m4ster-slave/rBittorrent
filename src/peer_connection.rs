use std::{
    io::{self, Read, Write},
    net::{SocketAddrV4, TcpStream},
    thread::{self},
    time::Duration,
};

use sha1::{Digest, Sha1};

use crate::parser::{calculate_info_hash_bytes, FileTree, Info};
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

fn download_piece(
    stream: &mut TcpStream,
    piece_index: u32,
    piece_length: usize,
) -> io::Result<Vec<u8>> {
    let block_size = 16 * 1024;
    let mut piece_buffer = vec![0u8; piece_length];

    // send requests
    for offset in (0..piece_length).step_by(block_size) {
        let block_len = if offset + block_size > piece_length {
            piece_length - offset
        } else {
            block_size
        };

        let mut request = Vec::with_capacity(13);
        request.push(6); // request message id
        request.extend_from_slice(&piece_index.to_be_bytes());
        request.extend_from_slice(&(offset as u32).to_be_bytes());
        request.extend_from_slice(&(block_len as u32).to_be_bytes());

        // write request length prefix + message
        let mut full_request = Vec::new();
        full_request.extend_from_slice(&(request.len() as u32).to_be_bytes());
        full_request.extend_from_slice(&request);
        stream.write_all(&full_request)?;
    }

    let mut received_bytes = 0;
    let mut u32_buf = [0u8; 4];

    while received_bytes < piece_length {
        // read length prefix
        stream.read_exact(&mut u32_buf).unwrap();
        let msg_len = u32::from_be_bytes(u32_buf);

        if msg_len == 0 {
            println!("Received keep-alive");
            continue;
        }

        let mut msg_buf = vec![0u8; msg_len as usize];
        stream.read_exact(&mut msg_buf).unwrap();

        let msg_id = msg_buf[0];
        let payload = &msg_buf[1..];

        match msg_id {
            7 => {
                // piece message
                let index = u32::from_be_bytes(payload[0..4].try_into().unwrap());
                let begin = u32::from_be_bytes(payload[4..8].try_into().unwrap());
                let block_data = &payload[8..];

                // Sanity check
                assert_eq!(index, piece_index);

                // insert block into piece buffer
                let offset = begin as usize;
                piece_buffer[offset..offset + block_data.len()].copy_from_slice(block_data);

                received_bytes += block_data.len();
            }
            0 => {
                // skip choke
                thread::sleep(Duration::new(0, 100_000)); // wait 100ms
            }
            _ => {
                // TODO handle other types
                // skip for now
                thread::sleep(Duration::new(0, 100_000));
            }
        }
    }

    Ok(piece_buffer)
}

#[derive(Debug)]
struct PieceHashMismatchError;

impl std::fmt::Display for PieceHashMismatchError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "The received piece hash doesn't match the hash in the file"
        )
    }
}

impl std::error::Error for PieceHashMismatchError {}

impl Peer {
    pub fn get_piece(
        &self,
        info_dict: &Info,
        index: usize,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // all messages follow <length prefix: 4 bytes><message ID: 1 byte><optional payload>

        // Step 1
        // perform handshake
        let infohash = calculate_info_hash_bytes(info_dict)?;
        let handshake = generate_handshake(&infohash);
        let mut stream = TcpStream::connect(self.sock_ip)?;
        let _ = stream.write(&handshake)?;

        // Step 2
        // read bitfield packaged
        //
        // 'bitfield' is only ever sent as the first message. Its payload is a bitfield with each
        // index that downloader has sent set to one and the rest set to zero. Downloaders which
        // don't have anything yet may skip the 'bitfield' message. The first byte of the bitfield
        // corresponds to indices 0 - 7 from high bit to low bit, respectively. The next one 8-15,
        // etc. Spare bits at the end are set to zero.
        let mut buf = vec![0u8; 68];
        stream.read_exact(&mut buf)?;

        // read in length first
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf)?;
        let msg_len = u32::from_be_bytes(len_buf);

        let mut msg_buf = vec![0u8; msg_len as usize];
        stream.read_exact(&mut msg_buf)?;

        let message_id = msg_buf[0];
        let payload = msg_buf[1..].to_vec();

        println!("Message ID: {}", message_id);
        println!(
            "bitfield: {:?}",
            payload
                .iter()
                .map(|b| format!("{:08b}", b))
                .collect::<Vec<_>>()
        );

        // Step 3
        // send interested packaged
        let mut msg_buf = Vec::new();
        // length prefix: 1
        msg_buf.extend_from_slice(&(1u32.to_be_bytes()));
        msg_buf.push(2);
        stream.write_all(&msg_buf)?;

        // Step 4
        // wait for unchoke message
        let mut msg_buf: Vec<u8> = vec![0];
        while msg_buf[0] != 1 {
            stream.read_exact(&mut len_buf)?;
            msg_buf = vec![0u8; u32::from_be_bytes(len_buf) as usize];
            stream.read_exact(&mut msg_buf)?;
            println!("waiting for unchoke message, got: {}", msg_buf[0]);
            // sleep 100ms to not overload the network
            thread::sleep(Duration::new(0, 100_000));
        }
        println!("Got unchoke message");

        // Step 5 & 6
        // send request messages and wait for piece messages putting all together

        // calculate correct piece size
        let total_pieces = info_dict.pieces.len() / 20;
        let total_length = match &info_dict.file_tree {
            FileTree::SingleFile { length } => length,
            FileTree::MultiFile { .. } => {
                unimplemented!()
            }
        };

        let piece_length = if index + 1 == total_pieces {
            total_length - (index * info_dict.piece_length)
        } else {
            info_dict.piece_length
        };

        println!("Downloading piece with length {}", piece_length);
        let piece = download_piece(&mut stream, index as u32, piece_length)?;

        // compare hash of piece with the hash in the info dict
        let mut hasher = Sha1::new();
        hasher.update(&piece);
        if hasher.finalize().to_vec() == info_dict.pieces[index * 20..index * 20 + 20] {
            Ok(piece)
        } else {
            Err(Box::new(PieceHashMismatchError))
        }
    }
}
