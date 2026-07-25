use anyhow::anyhow;
use serde::{Deserialize, Deserializer, Serialize};
use sha1::{Digest, Sha1};
use std::fmt::Display;
use std::fs::File;
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone, Deserialize)]
/// Metainfo files (also known as .torrent files)
pub struct Torrent {
    /// The URL of the tracker.
    // Splitting announce and announce_list like that is quick dirty code, I am assuming
    // announce_list[0][0] is the same URL as listed in anounce anyways I just haven found that
    // being talked about in the spec
    #[serde(deserialize_with = "deserialize_announce_url")]
    pub announce: AnnounceUrl,
    /// Optional list of tracker tiers
    /// each inner Vec is a tier, tried in order,
    /// tiers themselves are shuffled/tried in order per BEP 12.
    #[serde(default)]
    #[serde(rename(deserialize = "announce-list"))]
    #[serde(deserialize_with = "deserialize_nested_announce_list")]
    pub announce_list: Option<Vec<Vec<AnnounceUrl>>>,
    pub info: Info,
    /// Purely informational
    pub comment: Option<String>,
    #[serde(rename(deserialize = "created by"))]
    pub created_by: Option<String>,
    pub creation_date: Option<i64>,
    /// Indicates the character encoding used for string fields in the torrent
    pub encoding: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Info {
    /// A display name for the torrent. It is purely advisory.
    pub name: String,
    /// The number of bytes that each logical piece in the peer protocol refers to. I.e. it sets
    /// the granularity of piece, request, bitfield and have messages. It must be a power of two
    /// and at least 16KiB. Files are mapped into this piece address space so that each non-empty
    /// file is aligned to a piece boundary and occurs in the same order as in the file tree. The
    /// last piece of each file may be shorter than the specified piece length, resulting in an
    /// alignment gap.
    #[serde(rename(serialize = "piece length", deserialize = "piece length"))]
    pub piece_length: usize,
    /// Each entry is the SHA1 hash of the piece at the corresponding index. Should be a multiple
    /// of 20.
    #[serde(with = "serde_bytes")]
    pub pieces: Vec<u8>,
    #[serde(flatten)]
    pub file_tree: FileTree,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum AnnounceUrl {
    #[serde(rename = "http")]
    Http(String),
    #[serde(rename = "udp")]
    Udp(String),
    #[serde(rename = "wss")]
    Wss(String),
}

impl Display for AnnounceUrl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AnnounceUrl::Http(s) => write!(f, "{}", s),
            AnnounceUrl::Udp(s) => write!(f, "{}", s),
            AnnounceUrl::Wss(s) => write!(f, "{}", s),
        }
    }
}

impl AnnounceUrl {
    pub fn parse(s: &str) -> Result<Self, String> {
        if s.starts_with("http://") || s.starts_with("https://") {
            Ok(AnnounceUrl::Http(s.to_string()))
        } else if s.starts_with("udp://") {
            Ok(AnnounceUrl::Udp(s.to_string()))
        } else if s.starts_with("wss://") || s.starts_with("ws://") {
            Ok(AnnounceUrl::Wss(s.to_string()))
        } else {
            Err(format!("unsupported URL scheme: {s}"))
        }
    }
}

pub fn deserialize_announce_url<'de, D>(deserializer: D) -> Result<AnnounceUrl, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    AnnounceUrl::parse(&s).map_err(serde::de::Error::custom)
}

fn deserialize_nested_announce_list<'de, D>(
    deserializer: D,
) -> Result<Option<Vec<Vec<AnnounceUrl>>>, D::Error>
where
    D: Deserializer<'de>,
{
    let raw: Option<Vec<Vec<String>>> = Option::deserialize(deserializer)?;

    match raw {
        Some(outer) => {
            let mut result = Vec::new();
            for inner in outer {
                let parsed_inner = inner
                    .into_iter()
                    .map(|s| AnnounceUrl::parse(&s))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(serde::de::Error::custom)?;
                result.push(parsed_inner);
            }
            Ok(Some(result))
        }
        None => Ok(None),
    }
}

/// Distinguish between multi- and singlefile torrents as they need to be handles differently
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FileTree {
    /// Single file with `Torrent.name` as name
    SingleFile {
        /// Length of the file in bytes. Presence of this field indicates that the dictionary
        /// describes a file, not a directory. Which means it must not have any sibling entries.
        length: usize,
    },
    /// Set of files that go in a directory structure
    MultiFile { files: Vec<FileInfo> },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileInfo {
    /// Length of the file in bytes.
    pub length: usize,
    /// Subdirectory names
    pub path: Vec<String>,
}

// Parse the torretn file into a `Torrent` object
pub fn parse_torrent_file<P: AsRef<Path>>(path: P) -> Result<Torrent, anyhow::Error> {
    let mut file = File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let torrent: Torrent = serde_bencode::de::from_bytes(&buf)?;
    Ok(torrent)
}

/// Calculate info hash as a hex encoded string
pub fn calculate_info_hash(info_dict: &Info) -> Result<String, anyhow::Error> {
    Ok(hex::encode(calculate_info_hash_bytes(info_dict)?))
}

/// Calculate info hash as raw bytes string
pub fn calculate_info_hash_bytes(info_dict: &Info) -> Result<[u8; 20], anyhow::Error> {
    let bencoded_info_dict = serde_bencode::ser::to_bytes(info_dict)?;
    let mut hasher = Sha1::new();
    hasher.update(&bencoded_info_dict);
    Ok(hasher.finalize().into())
}

/// Get the hash of each piece in the metainfo file
pub fn get_pieces_hashes(info_dict: &Info) -> Result<Vec<String>, anyhow::Error> {
    // check if info hash is a multiple of 20
    if !info_dict.pieces.len().is_multiple_of(20) {
        return Err(anyhow!("`pieces` length is not a multiple of 20"));
    }

    let mut result = Vec::new();

    for i in 0..info_dict.pieces.len() / 20 {
        let bencoded_piece = &info_dict.pieces[i * 20..i * 20 + 20];
        result.push(hex::encode(bencoded_piece));
    }
    Ok(result)
}

impl Display for Torrent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Tracker URL: {}\nLength: {:?}\nInfo Hash {}\nPiece Length: {}\nPiece Hashes: \n{}\n",
            self.announce,
            self.info.file_tree,
            calculate_info_hash(&self.info).unwrap(),
            self.info.piece_length,
            get_pieces_hashes(&self.info).unwrap().join("\n"),
        )?;
        if let Some(list) = &self.announce_list {
            writeln!(f, "Announce list: {:?}", list)?;
        }
        if let Some(comment) = &self.comment {
            writeln!(f, "Comment: {}", comment)?;
        }
        if let Some(created_by) = &self.created_by {
            writeln!(f, "Created by: {}", created_by)?;
        }
        if let Some(creation_date) = &self.creation_date {
            writeln!(f, "Creation date: {}", creation_date)?;
        }
        if let Some(encoding) = &self.encoding {
            writeln!(f, "Encoding: {}", encoding)?;
        }
        Ok(())
    }
}
