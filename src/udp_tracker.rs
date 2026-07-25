//! Why UDP? this is what it says in BEP 15:
//!
//! > Using HTTP introduces significant overhead. There's overhead at the ethernet layer (14 bytes per packet), at the IP layer (20 bytes per packet), at the TCP layer (20 bytes per packet) and at the HTTP layer. About 10 packets are used for a request plus response containing 50 peers and the total number of bytes used is about 1206 [1]. This overhead can be reduced significantly by using a UDP based protocol. The protocol proposed here uses 4 packets and about 618 bytes, reducing traffic by 50%. For a client, saving 1 kbyte every hour isn't significant, but for a tracker serving a million peers, reducing traffic by 50% matters a lot. An additional advantage is that a UDP based binary protocol doesn't require a complex parser and no connection handling, reducing the complexity of tracker code and increasing it's performance.
use bytes::{Buf, BufMut};
use std::net::{Ipv4Addr, SocketAddrV4};

pub const PROTOCOL_ID: i64 = 0x41727101980;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum Action {
    Connect = 0,
    Announce = 1,
    Scrape = 2,
    Error = 3,
}

impl Action {
    pub fn from_i32(val: i32) -> Result<Self, anyhow::Error> {
        match val {
            0 => Ok(Action::Connect),
            1 => Ok(Action::Announce),
            2 => Ok(Action::Scrape),
            3 => Ok(Action::Error),
            other => Err(anyhow::anyhow!(format!("Unknown action: {}", other))),
        }
    }
}

/// BEP 15 defines the connect package as this:
/// ```
/// 0       64-bit integer  protocol_id     0x41727101980 // magic constant
/// 8       32-bit integer  action          0 // connect
/// 12      32-bit integer  transaction_id
/// 16
/// ```
/// 1. Construct this packet to send to tracker over udp
#[derive(Debug)]
pub struct ConnectRequest {
    protocol_id: i64,
    action: i32,
    transaction_id: i32,
}

impl ConnectRequest {
    pub fn new(transaction_id: i32) -> Self {
        Self {
            protocol_id: PROTOCOL_ID,
            action: Action::Connect as i32,
            transaction_id,
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(16);
        buf.put_i64(self.protocol_id); // Big-endian
        buf.put_i32(self.action); // Big-endian
        buf.put_i32(self.transaction_id); // Big-endian
        buf
    }
}

/// 2. Tracker response
/// Note the connection and transaction_id for later use
///
/// ```
///Offset  Size            Name            Value
/// 0       32-bit integer  action          0 // connect
/// 4       32-bit integer  transaction_id
/// 8       64-bit integer  connection_id
/// 16
/// ```
#[derive(Debug)]
pub struct ConnectResponse {
    action: Action,
    transaction_id: i32,
    pub connection_id: i64,
}

impl ConnectResponse {
    pub fn parse(mut src: &[u8]) -> Result<Self, anyhow::Error> {
        if src.len() < 16 {
            return Err(anyhow::anyhow!("Packet too short for ConnectResponse"));
        }

        let action = Action::from_i32(src.get_i32())?;
        let transaction_id = src.get_i32();
        let connection_id = src.get_i64();

        Ok(Self {
            action,
            transaction_id,
            connection_id,
        })
    }
}

/// ```
/// Offset  Size    Name    Value
/// 0       64-bit integer  connection_id
/// 8       32-bit integer  action          1 // announce
/// 12      32-bit integer  transaction_id
/// 16      20-byte string  info_hash
/// 36      20-byte string  peer_id
/// 56      64-bit integer  downloaded
/// 64      64-bit integer  left
/// 72      64-bit integer  uploaded
/// 80      32-bit integer  event           0 // 0: none; 1: completed; 2: started; 3: stopped
/// 84      32-bit integer  IP address      0 // default
/// 88      32-bit integer  key
/// 92      32-bit integer  num_want        -1 // default
/// 96      16-bit integer  port
/// 98
/// ```
#[derive(Debug)]
pub struct AnnounceRequest {
    connection_id: i64,
    action: i32,
    transaction_id: i32,
    info_hash: [u8; 20],
    peer_id: [u8; 20],
    downloaded: i64,
    left: i64,
    uploaded: i64,
    /// The event, one of
    /// none = 0
    /// completed = 1
    /// started = 2
    /// stopped = 3
    event: i32,
    ip_address: u32,
    /// A unique key that is randomized by the client.
    key: u32,
    num_want: i32,
    port: u16,
}

impl AnnounceRequest {
    pub fn new(
        connection_id: i64,
        transaction_id: i32,
        info_hash: [u8; 20],
        peer_id: [u8; 20],
        downloaded: i64,
        left: i64,
        uploaded: i64,
        port: u16,
    ) -> Self {
        Self {
            connection_id,
            action: Action::Announce as i32, // 1
            transaction_id,
            info_hash,
            peer_id,
            downloaded,
            left,
            uploaded,
            event: 2, // 2 = started
            ip_address: 0,
            key: rand::random::<u32>(),
            num_want: -1,
            port,
        }
    }
    pub fn serialize(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(98);
        buf.put_i64(self.connection_id);
        buf.put_i32(self.action);
        buf.put_i32(self.transaction_id);
        buf.put_slice(&self.info_hash);
        buf.put_slice(&self.peer_id);
        buf.put_i64(self.downloaded);
        buf.put_i64(self.left);
        buf.put_i64(self.uploaded);
        buf.put_i32(self.event);
        buf.put_u32(self.ip_address);
        buf.put_u32(self.key);
        buf.put_i32(self.num_want);
        buf.put_u16(self.port);
        buf
    }
}

/// ```
/// Offset      Size            Name            Value
/// 0           32-bit integer  action          1 // announce
/// 4           32-bit integer  transaction_id
/// 8           32-bit integer  interval
/// 12          32-bit integer  leechers
/// 16          32-bit integer  seeders
/// 20 + 6 * n  32-bit integer  IP address
/// 24 + 6 * n  16-bit integer  TCP port
/// 20 + 6 * N
/// ```
#[derive(Debug)]
pub struct AnnounceResponse {
    action: Action,
    transaction_id: i32,
    pub interval: i32,
    leechers: i32,
    seeders: i32,
    /// list of peer ips and ports
    pub peers: Vec<SocketAddrV4>,
}

impl AnnounceResponse {
    pub fn parse(mut src: &[u8]) -> Result<Self, anyhow::Error> {
        if src.len() < 20 {
            return Err(anyhow::anyhow!(
                "Packet too short for AnnounceResponse header"
            ));
        }

        let action = Action::from_i32(src.get_i32())?;
        let transaction_id = src.get_i32();
        let interval = src.get_i32();
        let leechers = src.get_i32();
        let seeders = src.get_i32();

        // The rest of the payload consists of 6-byte peer chunks: 4 bytes IP + 2 bytes Port
        let remaining_bytes = src.remaining();
        if remaining_bytes % 6 != 0 {
            return Err(anyhow::anyhow!("Invalid payload size for peer list"));
        }

        let mut peers = Vec::with_capacity(remaining_bytes / 6);
        while src.has_remaining() {
            let ip = Ipv4Addr::from(src.get_u32());
            let port = src.get_u16();
            peers.push(SocketAddrV4::new(ip.into(), port));
        }

        Ok(Self {
            action,
            transaction_id,
            interval,
            leechers,
            seeders,
            peers,
        })
    }
}

#[derive(Debug)]
pub struct TrackerError {
    action: Action,
    transaction_id: i32,
    pub error_string: String,
}

impl TrackerError {
    pub fn parse(mut src: &[u8]) -> Result<Self, anyhow::Error> {
        if src.len() < 8 {
            return Err(anyhow::anyhow!("Packet too short for TrackerError"));
        }

        let action = Action::from_i32(src.get_i32())?;
        let transaction_id = src.get_i32();
        let error_string = String::from_utf8_lossy(src).into_owned();

        Ok(Self {
            action,
            transaction_id,
            error_string,
        })
    }
}
