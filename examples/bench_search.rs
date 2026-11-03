//! Time the six-move search on a position: cargo run --release --example bench_search -- [reference <plies>] [fen]
use chess::board::{Board, START_FEN};
use chess::search::{full_search, full_search_reference, thousands, Progress};
fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (reference, rest) = if args.first().map(|s| s == "reference").unwrap_or(false) {
        (Some(args[1].parse::<usize>().unwrap()), &args[2..])
    } else {
        (None, &args[..])
    };
    let fen = if rest.is_empty() {
        START_FEN.to_string()
    } else {
        rest.join(" ")
    };
    let board = Board::from_fen(&fen).unwrap();
    let t = std::time::Instant::now();
    let r = match reference {
        Some(p) => full_search_reference(&board, &[], &Progress::default(), p),
        None => full_search(&board, &[], &Progress::default()),
    }
    .unwrap();
    println!(
        "{} nodes, {:.2} s on {} threads",
        thousands(r.total_nodes()),
        t.elapsed().as_secs_f64(),
        r.threads
    );
}
