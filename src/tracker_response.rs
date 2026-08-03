use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer};
use std::fmt;
use std::net::{Ipv4Addr, SocketAddrV4};

use crate::peer_connection::Peer;

#[derive(Debug)]
pub struct TrackerResponse {
    /// The number of seconds the downloader should wait between regular rerequests
    pub interval: i32,

    /// list of dictionaries corresponding to peers, each of which contains the keys peer id, ip,
    /// and port, which map to the peer's self-selected ID, IP address or dns name as a string, and
    pub peers: Vec<Peer>,
}

// im gonna be so real this is some serde wizardry i dont really understand but gemini does so
// yipiee
//
// from what i understand i need to do this because we parse the bencoded map and the peers which
// are binary arrays and the normal derive macro cant handle that
impl<'de> Deserialize<'de> for TrackerResponse {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        enum Field {
            Interval,
            Peers,
            Ignore,
        }

        struct FieldVisitor;
        impl<'de> Visitor<'de> for FieldVisitor {
            type Value = Field;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("field identifier")
            }

            fn visit_str<E>(self, v: &str) -> Result<Field, E> {
                match v {
                    "interval" => Ok(Field::Interval),
                    "peers" => Ok(Field::Peers),
                    _ => Ok(Field::Ignore),
                }
            }
        }

        impl<'de> Deserialize<'de> for Field {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: Deserializer<'de>,
            {
                deserializer.deserialize_identifier(FieldVisitor)
            }
        }

        struct PeerResponseVisitor;
        impl<'de> Visitor<'de> for PeerResponseVisitor {
            type Value = TrackerResponse;

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("a BitTorrent tracker response dictionary")
            }

            fn visit_map<V>(self, mut map: V) -> Result<TrackerResponse, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut interval = None;
                let mut peers = None;

                // 1 === this is the first thing thats done, serde loops thru keys?? and parses them
                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Interval => {
                            if interval.is_some() {
                                return Err(de::Error::duplicate_field("interval"));
                            }
                            interval = Some(map.next_value()?);
                        }
                        Field::Peers => {
                            if peers.is_some() {
                                return Err(de::Error::duplicate_field("peers"));
                            }
                            // Deserialize raw bytes directly from bencode binary string
                            let peer_bytes: serde_bytes::ByteBuf = map.next_value()?;

                            if !peer_bytes.len().is_multiple_of(6) {
                                return Err(de::Error::custom(
                                    "peers byte string length must be a multiple of 6",
                                ));
                            }

                            let parsed_peers = peer_bytes
                                .chunks_exact(6)
                                .map(|chunk| {
                                    let ip = Ipv4Addr::new(chunk[0], chunk[1], chunk[2], chunk[3]);
                                    let port = u16::from_be_bytes([chunk[4], chunk[5]]);
                                    Peer {
                                        sock_ip: SocketAddrV4::new(ip, port),
                                        available: Vec::new(),
                                        conn: None,
                                        peer_choking: true,
                                    }
                                })
                                .collect();

                            peers = Some(parsed_peers);
                        }
                        // skip through any extra fields
                        Field::Ignore => {
                            let _: de::IgnoredAny = map.next_value()?;
                        }
                    }
                }

                let interval = interval.ok_or_else(|| de::Error::missing_field("interval"))?;
                let peers = peers.ok_or_else(|| de::Error::missing_field("peers"))?;

                Ok(TrackerResponse { interval, peers })
            }
        }

        deserializer.deserialize_map(PeerResponseVisitor)
    }
}
