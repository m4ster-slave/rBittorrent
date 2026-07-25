# rBittorrent

==== WIP ====

Simple implementation of the BitTorrent protocoll in rust with minimal dependencies

Im doing this to learn about the Protocoll i use to download all my favorite shows and movies as i **LOVE** sailing the seas ^^

u can read the paper and writing notes here

- [PAPER](./paper/dist/main.pdf)
- [WRITING_NOTES](./paper/writing_notes.md)

## Learning Material

- https://www.bittorrent.org/beps/bep_0052.html
- https://app.codecrafters.io/courses/bittorrent/stages/ow9
- https://www.youtube.com/watch?v=jf_ddGnum_4&t=2062s
- https://bittorrent.org/bittorrentecon.pdf
- https://www.bittorrent.org/beps/bep_0005.html
- https://www.bittorrent.org/beps/bep_0000.html
- https://www.cs.cornell.edu/people/egs/714-spring05/bt-analysis.pdf
- https://web.mit.edu/6.829/www/currentsemester/papers/bittyrant.pdf
- https://dl.acm.org/doi/10.1145/1177080.1177106
- https://arxiv.org/abs/cs/0609026
- https://www.researchgate.net/figure/Comparison-of-random-small-world-and-scale-free-networks-Topological-structure-of_fig1_320308445

## TODO

- [ ] support for multiple files
- [ ] DHT
- [ ] Magnet links
- [ ] (Seeding)

- handshake all the peers to view their bitfield and choose peers for each piece
- create a queue of pieces
- each of those indexes create a ques of pieces and requests them async
- recollect and put into file
