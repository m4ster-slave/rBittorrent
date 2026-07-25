use anyhow::{anyhow, Ok};
use sha1::{Digest, Sha1};
use std::{net::SocketAddrV4, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
    sync::Mutex,
};

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
    /// Vector of booleans that are either set to true: meaning a piece is available or false:
    /// meaning a piece is not available
    pub available: Vec<bool>,
    pub conn: Option<Arc<Mutex<TcpStream>>>,
    /// per the spec, unchoke is a state, not a per-request event, sooooo that means if i keep
    /// waiting for new unchoke events i can wait untile the heat death of the universe
    pub peer_choking: bool,
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

async fn download_piece(
    stream: &mut TcpStream,
    piece_index: u32,
    piece_length: usize,
) -> Result<Vec<u8>, anyhow::Error> {
    const BLOCK_SIZE: usize = 16 * 1024;
    let mut piece_buffer = vec![0u8; piece_length];

    // send requests
    for offset in (0..piece_length).step_by(BLOCK_SIZE) {
        let block_len = if offset + BLOCK_SIZE > piece_length {
            piece_length - offset
        } else {
            BLOCK_SIZE
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
        stream.write_all(&full_request).await?;
    }

    let mut received_bytes = 0;
    let mut u32_buf = [0u8; 4];

    while received_bytes < piece_length {
        // read length prefix
        stream.read_exact(&mut u32_buf).await?;
        let msg_len = u32::from_be_bytes(u32_buf);

        if msg_len == 0 {
            println!("Received keep-alive");
            continue;
        }

        let mut msg_buf = vec![0u8; msg_len as usize];
        stream.read_exact(&mut msg_buf).await?;

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
                tokio::time::sleep(Duration::new(0, 100_000)).await; // wait 100ms
            }
            _ => {
                // TODO handle other types
                // skip for now
                tokio::time::sleep(Duration::new(0, 100_000)).await;
            }
        }
    }

    Ok(piece_buffer)
}

impl Peer {
    pub async fn perform_handshake(&mut self, info_dict: &Info) -> Result<(), anyhow::Error> {
        // all messages follow <length prefix: 4 bytes><message ID: 1 byte><optional payload>

        // Step 1
        // perform handshake
        let infohash = calculate_info_hash_bytes(info_dict)?;
        let handshake = generate_handshake(&infohash);
        self.conn = Some(Arc::new(Mutex::new(
            TcpStream::connect(self.sock_ip).await?,
        )));
        let conn = self.conn.as_ref().unwrap();
        let mut stream = conn.lock().await;
        stream.write_all(&handshake).await?;

        let mut buf = vec![0u8; 68];
        stream.read_exact(&mut buf).await?;

        if buf[0] != 19 || &buf[1..20] != b"BitTorrent protocol" {
            return Err(anyhow!(
                "The received handshake is not BitTorrent handshake",
            ));
        }

        if buf[28..48] != infohash {
            return Err(anyhow!(
                "The received handshake doesnt match the handshake generated by the client",
            ));
        }

        // Step 2
        // read bitfield packaged
        //
        // 'bitfield' is only ever sent as the first message. Its payload is a bitfield with each
        // index that downloader has sent set to one and the rest set to zero. Downloaders which
        // don't have anything yet may skip the 'bitfield' message. The first byte of the bitfield
        // corresponds to indices 0 - 7 from high bit to low bit, respectively. The next one 8-15,
        // etc. Spare bits at the end are set to zero.
        // read in length first
        let mut len_buf = [0u8; 4];
        stream.read_exact(&mut len_buf).await?;
        let msg_len = u32::from_be_bytes(len_buf);

        let mut msg_buf = vec![0u8; msg_len as usize];
        stream.read_exact(&mut msg_buf).await?;

        let _message_id = msg_buf[0];
        let bitfield = msg_buf[1..].to_vec();
        println!("Bits: ");
        for byte in &bitfield {
            for i in (0..8).rev() {
                let bit = (byte >> i) & 1;
                print!("{bit}, ");
                if bit == 1 {
                    self.available.push(true);
                } else {
                    self.available.push(false);
                }
            }
        }

        Ok(())
    }

    pub async fn get_piece(
        &mut self,
        info_dict: &Info,
        index: usize,
    ) -> Result<Vec<u8>, anyhow::Error> {
        let conn = self.conn.as_ref().unwrap();
        let mut stream = conn.lock().await;

        let mut len_buf = [0u8; 4];

        if self.peer_choking {
            // Step 1: send interested
            let mut msg_buf = Vec::new();
            msg_buf.extend_from_slice(&(1u32.to_be_bytes()));
            msg_buf.push(2);
            stream.write_all(&msg_buf).await?;

            // Step 2: wait for unchoke
            loop {
                stream.read_exact(&mut len_buf).await?;
                let msg_len = u32::from_be_bytes(len_buf);

                if msg_len == 0 {
                    println!("Received keep-alive while waiting for unchoke");
                    tokio::time::sleep(Duration::from_millis(100)).await;
                    continue;
                }

                let mut msg_buf = vec![0u8; msg_len as usize];
                stream.read_exact(&mut msg_buf).await?;
                let msg_id = msg_buf[0];

                match msg_id {
                    1 => {
                        self.peer_choking = false;
                        break;
                    }
                    0 => {
                        self.peer_choking = true; // stays choked
                    }
                    _ => {
                        // have, bitfield, etc. — ignore for now but don't misinterpret them
                    }
                }

                tokio::time::sleep(Duration::from_millis(100)).await;
            }
            println!("Got unchoke message");
        } else {
            println!("Already unchoked, skipping interested/unchoke handshake");
        }

        // Step 3 & 4
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
        let piece = download_piece(&mut stream, index as u32, piece_length).await?;
        println!("Piece succesfully downloaded");

        // compare hash of piece with the hash in the info dict
        let mut hasher = Sha1::new();
        hasher.update(&piece);
        if hasher.finalize().to_vec() == info_dict.pieces[index * 20..index * 20 + 20] {
            println!("Piece hash matches the hash in the file");
            Ok(piece)
        } else {
            Err(anyhow!(
                "The received piece hash doesn't match the hash in the file"
            ))
        }
    }
}
