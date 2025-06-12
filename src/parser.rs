#![allow(dead_code)]
use serde::{Deserialize, Serialize};
use serde_bencode::de;
use serde_bencode::ser;
use sha1::{Digest, Sha1};
use std::error::Error;
use std::fmt::Display;
use std::fs::File;
use std::io::Read;
use std::path::Path;
use url::form_urlencoded;

#[derive(Debug, Clone, Deserialize)]
/// Metainfo files (also known as .torrent files)
pub struct Torrent {
    /// The URL of the tracker.
    pub announce: String,
    pub info: Info,
    pub comment: Option<String>,
    #[serde(rename(deserialize = "created by"))]
    pub created_by: Option<String>,
    pub creation_date: Option<i64>,
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
    /// Each entry is the SHA1 hash of the piece at the corresponding index.
    #[serde(with = "serde_bytes")]
    pub pieces: Vec<u8>,
    #[serde(flatten)]
    pub file_tree: FileTree,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum FileTree {
    /// single file with `Torrent.name` as name
    SingleFile {
        /// Length of the file in bytes. Presence of this field indicates that the dictionary
        /// describes a file, not a directory. Which means it must not have any sibling entries.
        length: usize,
    },
    /// set of files that go in a directory structure
    MultiFile { files: Vec<FileInfo> },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct FileInfo {
    /// Length of the file in bytes.
    pub length: usize,
    /// Subdirectory names
    pub path: Vec<String>,
}

pub fn parse_torrent_file<P: AsRef<Path>>(path: P) -> Result<Torrent, Box<dyn std::error::Error>> {
    let mut file = File::open(path)?;
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let torrent: Torrent = de::from_bytes(&buf)?;
    Ok(torrent)
}

pub fn calculate_info_hash(info_dict: &Info) -> Result<String, Box<dyn std::error::Error>> {
    let bencoded_info_dict = ser::to_bytes(info_dict)?;
    let mut hasher = Sha1::new();
    hasher.update(&bencoded_info_dict);
    let result = hasher.finalize();
    Ok(hex::encode(result))
}

pub fn calculate_info_hash_bytes(info_dict: &Info) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let bencoded_info_dict = ser::to_bytes(info_dict)?;
    let mut hasher = Sha1::new();
    hasher.update(&bencoded_info_dict);
    Ok(hasher.finalize().to_vec())
}

pub fn calculate_urlencoded_info_hash(
    info_dict: &Info,
) -> Result<String, Box<dyn std::error::Error>> {
    let bencoded_info_dict = ser::to_bytes(info_dict)?;
    let mut hasher = Sha1::new();
    hasher.update(&bencoded_info_dict);
    let result = hasher.finalize();

    let url_encoded_infohash = form_urlencoded::byte_serialize(&result).collect::<String>();
    Ok(url_encoded_infohash)
}

#[derive(Debug)]
pub enum PiecesError {
    InvalidLength,
}

impl Display for PiecesError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PiecesError::InvalidLength => write!(f, "`pieces` length is not a multiple of 20"),
        }
    }
}

impl Error for PiecesError {}

pub fn get_pieces_hashes(info_dict: &Info) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    // check if info hash is a multiple of 20
    if info_dict.pieces.len() % 20 != 0 {
        return Err(Box::new(PiecesError::InvalidLength));
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
            "Tracker URL: {}\nLength: {:?}\nInfo Hash {}\nPiece Length: {}\nPiece Hashes: \n{}",
            self.announce,
            self.info.file_tree,
            calculate_info_hash(&self.info).unwrap(),
            self.info.piece_length,
            get_pieces_hashes(&self.info).unwrap().join("\n")
        )
    }
}
