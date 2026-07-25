#import "@preview/charged-ieee:0.1.4": ieee
#import "@preview/fletcher:0.5.8" as fletcher: diagram, edge, node, shapes

#show: ieee.with(
  title: [Evaluating Peer & Piece Selection Algorithms in the BitTorrent Protocol],
  abstract: [
    I present an empirical study on the performance of custom piece and peer
    selection heuristics in BitTorrent swarms using a custom Rust client.
  ],
  authors: (
    (
      name: "Lukiana Weger",
      department: [Student],
      organization: [],
      location: [],
      email: "mail@weger.dev",
    ),
  ),
  index-terms: ("BitTorrent", "Peer-to-Peer", "Algorithms", "Network Performance"),
  bibliography: bibliography("refs.bib", style: "ieee"),
  paper-size: "a4",
)

#let definition(term, content) = [
  #block(
    stroke: (left: 2pt + rgb("#c17817")),
    width: 100%,
    inset: (left: 4pt, rest: 5pt),
  )[
    #text(weight: "bold", fill: rgb("#8b5a0a"))[Definition: #term] \
    #content
  ]
]

#let theorem(name, content) = [
  #block(
    stroke: (left: 2pt + rgb("#2f7c4f")),
    width: 100%,
    inset: (left: 4pt, rest: 5pt),
  )[
    #text(weight: "bold", fill: rgb("#1f5938"))[Theorem: #name] \
    #content
  ]
]

#let example(content) = [
  #block(
    stroke: (left: 2pt + rgb("#7c5295")),
    width: 100%,
    inset: (left: 4pt, rest: 5pt),
  )[
    #text(weight: "bold", fill: rgb("#5a3a6f"))[Beispiel] \
    #content
  ]
]

#let note(content) = [
  #block(
    stroke: (left: 2pt + rgb("#3a72a8")),
    width: 100%,
    inset: (left: 4pt, rest: 5pt),
  )[
    #text(weight: "bold", fill: rgb("#2a5278"))[Wichtig:] #content
  ]
]

#outline()

= Introduction
In peer-to-peer networks, piece selection directly impacts performance as described in @cohen2003bittorrent[p. 3, 2.4], the optimistic unchoking slot...
#cite(<qiu2004bittorrent>, form: none)


#pagebreak()
#figure(
  scope: "parent",
  placement: auto,
  caption: [Hybrid UML Activity & Sequence diagram of the BitTorrent protocol. Swimlanes separate Client, Tracker, and Peer domains, highlighting network message exchanges and retry control loops.],
  [
    #set text(size: 7.5pt)
    #diagram(
      spacing: (65pt, 13pt),
      node-stroke: 0.8pt,
      edge-stroke: 0.8pt,
      node-corner-radius: 2pt,
      mark-scale: 100%,

      // SWIMLANE Row 0 - keep relevant actions at respective columns pls
      node((0, 0), [*CLIENT (LOCAL NODE)*], fill: rgb("e6f2ff"), corner-radius: 3pt, stroke: 0.5pt + blue),
      node((1, 0), [*TRACKER / DHT*], fill: rgb("e6ffe6"), corner-radius: 3pt, stroke: 0.5pt + green),
      node((2, 0), [*REMOTE PEER*], fill: rgb("fff2e6"), corner-radius: 3pt, stroke: 0.5pt + orange),

      node((0, 1), shape: shapes.circle, fill: black, radius: 5pt),
      edge("-|>"),

      node((0, 2), align(center)[Load .torrent / \ Magnet Link]),

      edge((0, 2), (1, 2), "-|>", [Announce info_hash], label-pos: 0.5),
      node((1, 2), align(center)[Register Client & \ Query Swarm  \ (DHT or Tracker)]),

      edge((1, 2), (0, 3), "-|>", [Get Peer List], label-pos: 0.5, label-side: center),

      node((0, 3), align(center)[Select Peer & \ Initiate Connection]),

      edge((0, 3), (2, 3), "-|>", [Handshake (info_hash)], label-pos: 0.5),

      node((2, 3), align(center)[Does info_hash \ match?], shape: shapes.diamond),

      edge((2, 3), (3, 3), "-|>", [No], label-pos: 0.4),
      node((3, 3), align(center)[Drop Conn.]),
      edge((3, 3), (3, 1), "-", label-pos: 0.4),
      edge((3, 1), (1.5, 1), "-", label-pos: 0.4),
      edge((1.5, 1), (1.5, 4), "-", label-pos: 0.4),
      edge((1.5, 4), (0, 4), "-", label-pos: 0.4),

      edge((2, 3), (2, 4.5), "-|>", [Yes], label-pos: 0.4),
      node((2, 4.5), align(center)[Bitfield & \ Handshake Response]),
      edge((2, 4.5), (0, 5), "-|>", [1. Send Bitfield], label-pos: 0.5),

      node((0, 5), align(center)[Send 'Interested' \ State]),
      edge((0, 5), (2, 5.5), "-|>", [2. Interested], label-pos: 0.5),

      node((2, 5.5), align(center)[Unchoke received?], shape: shapes.diamond),

      edge((2.4, 5.5), (2.5, 5.5), "-", label-pos: 0.4),
      edge((2.5, 5.5), (2.5, 5), "-", [No (Timeout)], label-pos: 0.4),
      edge((2.5, 5), (2, 5), "-", label-pos: 0.4),
      edge((2, 5), (2, 5.6), "-|>", label-pos: 0.4),

      edge((2, 5.5), (0, 6), "-|>", [4. Yes (Unchoke)], label-pos: 0.5),

      node((0, 6), align(center)[Request Piece \ Block]),
      edge((0, 6), (2, 6.5), "-|>", [6. Request(index, begin)], label-pos: 0.5),

      node((2, 6.5), align(center)[Transmit \ Block Data]),
      edge((2, 6.5), (0, 7), "-|>", [7. Piece Data], label-pos: 0.5),

      node((0, 7), align(center)[Verify SHA-1 \ Hash], shape: shapes.diamond),

      edge((0, 7), (-0.85, 7), "-", label-pos: 0.5, label-side: center),
      edge((-0.85, 7), (-0.85, 4), "-", [Invalid], label-pos: 0.5, label-side: center),
      edge((0, 7), (0, 9), "-|>", [Valid], label-pos: 0.4),

      node((0, 9), align(center)[More pieces from \ this peer?], shape: shapes.diamond),
      edge((0, 9), (-0.7, 9), "-"),
      edge((-0.7, 9), (-0.7, 6), [Yes], "-", label-pos: 0.1),
      edge((-0.7, 6), (0, 6), "-|>"),
      edge((0, 9), (0, 11), "-|>", [No], label-pos: 0.4),

      node((0, 11), align(center)[Finished \ downloading?], shape: shapes.diamond),
      edge("l,u,u,u,u,u,u,u,r", (0, 3), "-|>", [No], label-pos: 0.2),

      edge((0, 11), (0, 12.5), "-|>", [Yes], label-pos: 0.4),
      node((0, 12.5), align(center)[Seed (Upload Only)]),
      edge("-|>"),

      node(
        (0, 14),
        align(center + horizon)[#circle(radius: 3pt, fill: black)],
        shape: shapes.circle,
        fill: white,
        radius: 6pt,
        stroke: 1.2pt,
      ),
    )
  ],
) <fig-bittorrent-protocol>

= Quick Overview of the BitTorrent Protocol

As shown in @fig-bittorrent-protocol, control flow cycles between block transfers and peer re-selection





== The difference between DHT and Tracker
== _.torrent_ files and magnet links
== info_hash
== Clients and Pieces

The actual peer- and piece selection algorithms used by the BitTorrent protocl are random peer selection and a 4 phase piece selection algorithm as mentioned in @cohen2003bittorrent[p. 3, 2.4].

The peers get chosen at random because

These 4 phases of piece selection are:
+ Strict priority: Once a block of a piece is received all the other blocks will be downloaded first before another piece is chosen. My Implementation will do the same ting since i dont see why diverging from that practice could ever increse performance or swarm health.
+ The Client begins by selecting random pieces to download, until at least one piece is assembled.
+ It then switches to rearest first. Each peer only sees piece rareness within its own peer set not the global swarm.
+ Towards the end we switch into something called "Endgame mode". Once all the pieces are requested but the client is missing some block, it sends out requests for all the missing blocks to all _all_ connected peers.


= Piece Selection
== Rarest-First
=== Entropymaxxing
At each step pick whichever piece maximizes Shannon entropy of your local bitfield. Computationally absurd, probably converges to something close to rarest-first anyway, but the journey is the point.

#theorem("Shannon entropy")[
  Shannon entropy in its general form measures the average unpredictability of a probability distribution
  $ H(X) = -sum_(x in X ) p(x) dot log p(x) $

  For a bianry string of length $N$ as we have in our bitfield the naive application is jsut trating it as a Bernoulli distribution $->$ what fraction of bits are ones ($p_1$), and which are zeros ($p_0$).
  $ p_1 = ("number of pieces you have") / N $
  $ p_0 = 1 - p_1 $

  which gives us

  $ H_"naive" = -(p_1·log(p_1) + p_0·log(p_0)) $
]

Our naive approach will peak at $p_(1 or 2)0.5$ and will have zero at both extreme points. This is almost useless for selecting new pieces because it just tells us about the current entropy of the bitfield and not _which_ pieces you have. (`101010...` & `111000...` will have the same entropy).

What will actually give you a meaningfull value is is the joint distribution of local and swarm bitfield. for each piece $i$ define, $p(i) = "fraction of peers in the swarm that have piece i"$. Using this technique your local bitmap acts like a mask revealing the pieces that arent present in the local bitmask($S$), among those you want to pick the piece that maximizes the amount of new information you gain (of course relative to what the swarm can provide). A reasonable objective would be to take the piece with the highest surprise value out of a set defined as:

$ I = {-log_2(p_i) bar i in S} $

This will literally just result in being rarest-first reframed in information-theoretic language, which is a neat result because it proves that the rarest-first approach actually implicitly maximizes entropy.

Another interpratation of maximizing entropy could be maximizing local entropy by defining some criteria by which the bit field maximizes entropy, this for me is a arbirary criteria like "most spread out" and has no scientific value. If we follow this thought: you just use something like run-length-entropy (RLE) by which a bitfield like `11110000` has low run-length entropy (two long runs). A bitfield like `10100101` has higher run-length entropy (many short runs). Maximizing this pushes you toward scattered, interleaved piece acquisition. $->$ practically this pushes you towards something like random piece selectin with a sort of repulsion effect.

"Entropymaxxing" is theoretically elegant and practically reduces down to rarest-first or pretty much just randomized selection depending on the defenition. The computing overhead added by calculating the Shannon Entropy of any bitfield doesnt make this any viable competitor what so ever.





== Most common first
literally the inverse of rarest-first. Greedily download what everyone has. Swarm health collapses hilariously fast; good stress test for how badly broken a strategy can get.
== Random
=== Random walk with momentum
pick a random piece, then with 80% probability pick an adjacent piece next, else re-randomize. Creates weird locality clusters in the bitfield

== Sequential
=== Reverse sequential
== Hybrid rarest-first + sequential
what qBittorrent does for streaming) — rarest-first globally but sequential within a sliding window. You currently treat these as mutually exclusive.
== Availability-weighted random
weighted random where weights are inversely proportional to piece availability. Middle ground between pure random and strict rarest-first; avoids the "everyone rushes the rarest" thundering herd.




= Peer Selection
== Tit-for-Tat
== Anti-tit-for-tat / Altruism maximization
preferentially unchoke peers who upload the least to you. Will get you exploited by every free-rider in the swarm; interesting to see how fast
== Optimistic
== Latency-aware selection
preferring peers with lower RTT vs. purely tit-for-tat. Does proximity win over reciprocation? Especially relevant in sparse swarms.
== BAR (Bounded Altruism with Reciprocity)
game-theoretic model that tolerates some free-riding up to a bound. Interesting to benchmark against strict tit-for-tat in mixed honest/freeloading swarms.

== Gradient descent on peer utility
model each peer as having a utility score ($"upload rate" times "availability" times "latency"^(-1)$), gradient-descend the unchoke allocation every round. Probably overkill but defensible.

== Random
=== Unchoke by peer-id lexicographic order
since peer_id are often semi-random in the first place this should be roughly equivalent to random with extra steps.



= Block & Pipelining Parameters
== In-flight Queue Depth
BitTorrent requests pieces in smaller 16 KiB blocks. How many block requests are kept in-flight per peer (e.g., 16, 128, 250)? Lower values underutilize bandwidth; higher values increase latency or cause buffer bloat.

== Block Request Size
Standard is 16 KiB, but what happens to TCP/µTP overhead and throughput if you test 8 KiB, 32 KiB, or 64 KiB blocks?

== Endgame Mode Trigger Threshold
When a download is nearly finished (e.g., last 20 blocks or last 1–2%), clients send duplicate requests to all peers to finish quickly. Tuning when and how aggressively to enter endgame mode affects network overhead vs. completion tail-latency.



= Peer Dynamics & Choking Parameters
== Unchoke Round Frequency
Standard BitTorrent re-evaluates top downloading peers every 10 seconds and rotates optimistic unchokes every 30 seconds. What happens if you run choking rounds every 2s vs 15s?

== Unchoke Slot Count
Fixed number of active upload slots (e.g., 4 or 8 slots) versus dynamic bandwidth-based allocation per peer.

== Anti-Snubbing Sensitivity
If a peer doesn't send a block for $X$ seconds (standard \~60s), mark them as "snubbed" and stop uploading to them.

= Transport & Infrastructure
== Async Verification Pipeline
Does offloading SHA-1/SHA-256 piece hash checks to a background worker pool impact disk I/O bottlenecks compared to inline verification?
