use std::sync::Arc;
use tokio::sync::Mutex;

use crate::{discovery::PeerDiscoverer, parser::Torrent};

use tokio::{
    task::JoinSet,
    time::{self, Duration},
};

/// Responsible for downloading the file
pub struct Downloader {
    discoverer: PeerDiscoverer,
    pub file_buffer: Vec<u8>,
    torrent: Torrent,
}

impl Downloader {
    pub fn new(discoverer: &PeerDiscoverer, torrent: &Torrent) -> Self {
        Self {
            discoverer: discoverer.clone(),
            file_buffer: Vec::new(),
            torrent: torrent.clone(),
        }
    }

    pub async fn download(&mut self) {
        let total_pieces = self.torrent.info.pieces.len() / 20;

        let mut initial_queue = Vec::with_capacity(total_pieces);
        for i in 0..total_pieces {
            initial_queue.push(i);
        }
        let work_queue = Arc::new(Mutex::new(initial_queue));

        let total_length = match &self.torrent.info.file_tree {
            crate::parser::FileTree::SingleFile { length } => *length,
            crate::parser::FileTree::MultiFile { files } => files.iter().map(|f| f.length).sum(),
        };

        let shared_buffer = Arc::new(Mutex::new(vec![0u8; total_length]));
        let torrent_info = Arc::new(self.torrent.info.clone());

        let mut interval = time::interval(Duration::from_secs(1));
        let mut active_tasks = JoinSet::new();

        println!(
            "Starting download of {} pieces ({} bytes total)",
            total_pieces, total_length
        );

        loop {
            {
                let queue = work_queue.lock().await;
                if queue.is_empty() && active_tasks.is_empty() {
                    println!("All pieces downloaded successfully!");
                    break;
                }
            }

            tokio::select! {
                _ = interval.tick() => {

                    let discovery = self.discoverer.discover(&self.torrent).await.unwrap_or_else(|e| {
                        println!("Discovery service threw an error: {}", e);
                        panic!()
                    });

                    println!("Peers updated: {} peers", discovery.peers.len());

                    discovery.peers.iter().for_each(|p| println!("{}", p));

                    interval = time::interval(Duration::from_secs(discovery.interval as u64));

                    for mut peer in discovery.peers {

                        let queue_clone = Arc::clone(&work_queue);
                        let buffer_clone = Arc::clone(&shared_buffer);
                        let info_clone = Arc::clone(&torrent_info);

                        active_tasks.spawn(async move
                        {
                            loop {
                                // Pop a piece from the queue
                                // TODO piece selection algorithm
                                let piece_index = {
                                    let mut q = queue_clone.lock().await;
                                    match q.pop() {
                                        Some(idx) => idx,
                                        None => break,
                                    }
                                };


                                match peer.get_piece(&info_clone, piece_index).await {
                                    Ok(piece_data) => {
                                        let offset = piece_index * info_clone.piece_length;

                                        let mut buf = buffer_clone.lock().await;
                                        let end = (offset + piece_data.len()).min(buf.len());
                                        buf[offset..end].copy_from_slice(&piece_data[..end - offset]);

                                        println!("Successfully downloaded piece {} from {}", piece_index, peer.sock_ip);
                                    }
                                    Err(e) => {
                                        println!("Peer {} failed piece {}: {}", peer.sock_ip, piece_index, e);

                                        // put the piece back in the queue so another peer can try it
                                        queue_clone.lock().await.push(piece_index);

                                        // The peer threw an error (likely a disconnect or bad hash), so we kill this task
                                        break;
                                    }
                                }
                            }
                        });
                    }
                }
                // reap completed or failed peer tasks
                Some(res) = active_tasks.join_next(), if !active_tasks.is_empty() => {
                    if let Err(e) = res {
                        println!("A peer task panicked or failed: {:?}", e);
                    }
                }
            }
        }

        // Extract our completed byte stream from the Arc<Mutex>
        self.file_buffer = Arc::try_unwrap(shared_buffer)
            .expect("Fatal error: Multiple references to the file buffer still exist")
            .into_inner();
    }
}
