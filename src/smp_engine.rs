use std::collections::VecDeque;
use std::i8::MAX;
use std::sync::{Arc, mpsc};
use std::sync::atomic::{AtomicBool, Ordering};
use dashmap::DashMap;
use crate::bitboard::{Board, COLS, TOTAL_CELLS};

/// Default move ordering - center-first.
const DEFAULT_MOVE_ORDER: [u32; COLS as usize] = [3, 2, 4, 1, 5, 0, 6];

/// Number of helper threads
/// Total threads = HELPER_THREADS + 1 (master).
const HELPER_THREADS: usize = 11;

const TRANSPOSITION_TABLE_SIZE: usize = 1 << 20;
const TRANSPOSITION_TABLE_MASK: u64 = (TRANSPOSITION_TABLE_SIZE - 1) as u64;
const TRANSPOSITION_TABLE_ALL_WAY_MAX_DEPTH: u32 = 18;

const MIN_EVAL: i8 = -MAX_EVAL;
const MAX_EVAL: i8 = (TOTAL_CELLS << 1) as i8;


#[inline]
fn win_eval(moves_played: u32) -> i8 {
    MAX_EVAL - moves_played as i8
}


#[inline]
fn draw_eval(moves_played: u32, is_engine_first: bool) -> i8 {
    let first_player_turn = (moves_played & 1) == 0;
    let is_engine_turn = (is_engine_first && first_player_turn)
        || (!is_engine_first && !first_player_turn);

    if is_engine_turn {
        moves_played as i8
    } else {
        -(moves_played as i8)
    }
}


#[derive(Clone, Copy, PartialEq, Eq)]
enum Bound {
    Lower(i8),
    Upper(i8),
    Range(i8, i8),
}


type SharedCache = Arc<DashMap<u64, Bound>>;

fn shared_cache_get(cache: &DashMap<u64, Bound>, key: u64) -> Option<Bound> {
    cache.get(&key).map(|e| *e)
}

fn shared_cache_store(cache: &DashMap<u64, Bound>, key: u64, bound: Bound) {
    cache.entry(key).and_modify(|existing| {
        *existing = match (*existing, bound) {
            (Bound::Range(low, high), Bound::Lower(s)) => Bound::Range(s.max(low), high),
            (Bound::Range(low, high), Bound::Upper(s)) => Bound::Range(low, s.min(high)),
            (Bound::Lower(low),     Bound::Upper(high)) => Bound::Range(low, high),
            (Bound::Upper(high),     Bound::Lower(low)) => Bound::Range(low, high),
            (Bound::Lower(a),      Bound::Lower(b))  => Bound::Lower(a.max(b)),
            (Bound::Upper(a),      Bound::Upper(b))  => Bound::Upper(a.min(b)),
            _ => bound,
        };
    }).or_insert(bound);
}


#[derive(Clone, Copy)]
struct ArrayEntry {
    key:   u64,
    bound: Bound,
}

struct ArrayCache {
    entries: Vec<ArrayEntry>,
}

impl ArrayCache {
    fn new() -> Self {
        ArrayCache {
            entries: vec![ArrayEntry { key: 0, bound: Bound::Lower(0) }; TRANSPOSITION_TABLE_SIZE],
        }
    }

    #[inline]
    fn index(key: u64) -> usize {
        (key & TRANSPOSITION_TABLE_MASK) as usize
    }

    #[inline]
    fn store(&mut self, key: u64, bound: Bound) {
        let idx = Self::index(key);
        self.entries[idx] = ArrayEntry { key, bound };
    }

    #[inline]
    fn get(&self, key: u64) -> Option<Bound> {
        let entry = self.entries[Self::index(key)];
        if entry.key == key { Some(entry.bound) } else { None }
    }
}


struct ThreadLocalTranspositionTable {
    shared:      SharedCache,
    array:       ArrayCache,
    all_way_max_depth: u32,
}

impl ThreadLocalTranspositionTable {
    #[inline]
    fn store(&mut self, key: u64, bound: Bound, moves_played: u32) {
        if moves_played <= self.all_way_max_depth {
            shared_cache_store(&self.shared, key, bound);
        } else {
            self.array.store(key, bound);
        }
    }

    #[inline]
    fn get(&self, key: u64, moves_played: u32) -> Option<Bound> {
        if moves_played <= self.all_way_max_depth {
            shared_cache_get(&self.shared, key)
        } else {
            self.array.get(key)
        }
    }
}


fn evaluate_position(
    board: &Board,
    mut alpha: i8,
    mut beta: i8,
    transposition_table: &mut ThreadLocalTranspositionTable,
    node_count: &mut u64,
    stop: &AtomicBool,
) -> i8 {
    if stop.load(Ordering::Relaxed) {
        return alpha;
    }

    *node_count += 1;

    let curr_conn4 = board.current_player_connect4();
    let opp_conn4  = board.last_player_connect4();

    if curr_conn4 && opp_conn4 { return draw_eval(board.moves_played, board.is_engine_first); }
    if curr_conn4 { return win_eval(board.moves_played - 2); }
    if opp_conn4  { return -win_eval(board.moves_played - 1); }

    // must come after checks above
    if board.moves_played == TOTAL_CELLS { return draw_eval(TOTAL_CELLS, board.is_engine_first); }

    for next in board.next_positions_ordered(&DEFAULT_MOVE_ORDER) {
        if !next.current_player_connect4() && next.last_player_connect4() {
            return win_eval(board.moves_played);
        }
    }

    alpha = alpha.max(-win_eval(board.moves_played + 1));
    beta  = beta.min(win_eval(board.moves_played + 2));
    if alpha >= beta {
        return beta;
    }

    let key = board.key();

    if let Some(bound) = transposition_table.get(key, board.moves_played) {
        match bound {
            Bound::Lower(s)      => alpha = alpha.max(s),
            Bound::Upper(s)      => beta  = beta.min(s),
            Bound::Range(low, high) => { alpha = alpha.max(low); beta = beta.min(high); }
        }
        if alpha >= beta {
            return alpha;
        }
    }

    for next in board.next_positions_ordered(&DEFAULT_MOVE_ORDER) {
        if let Some(bound) = transposition_table.get(next.key(), next.moves_played) {
            let upper = match bound {
                Bound::Upper(high) | Bound::Range(_, high) => Some(high),
                _ => None,
            };
            if let Some(high) = upper {
                alpha = alpha.max(-high);
                if alpha >= beta {
                    return alpha;
                }
            }
        }
    }

    let mut first_move = true;

    for next in board.next_positions_ordered(&DEFAULT_MOVE_ORDER) {
        let eval = if first_move {
            first_move = false;
            -evaluate_position(
                &next,
                -beta,
                -alpha,
                transposition_table,
                node_count,
                stop
            )
        } else {
            let null_eval = -evaluate_position(
                &next,
                -alpha - 1,
                -alpha,
                transposition_table,
                node_count,
                stop
            );
            if null_eval > alpha && null_eval < beta {
                -evaluate_position(
                    &next,
                    -beta,
                    -null_eval,
                    transposition_table,
                    node_count,
                    stop
                )
            } else {
                null_eval
            }
        };

        if stop.load(Ordering::Relaxed) {
            return alpha;
        }

        if eval >= beta {
            transposition_table.store(key, Bound::Lower(eval), board.moves_played);
            return eval;
        }
        if eval > alpha {
            alpha = eval;
        }
    }

    transposition_table.store(key, Bound::Upper(alpha), board.moves_played);
    alpha
}


fn expand_starting_positions(root: &Board, n: usize) -> Vec<Board> {
    let mut frontier: VecDeque<Board> = VecDeque::new();

    for child in root.next_positions_ordered(&DEFAULT_MOVE_ORDER) {
        frontier.push_back(child);
    }

    while frontier.len() < n {
        match frontier.pop_front() {
            None => break,
            Some(board) => {
                let children: Vec<Board> = board
                    .next_positions_ordered(&DEFAULT_MOVE_ORDER)
                    .collect();
                if children.is_empty() {
                    frontier.push_back(board);
                    break;
                }
                for child in children {
                    frontier.push_back(child);
                }
            }
        }
    }

    frontier.into_iter().take(n).collect()
}

/// Sent from SmpEngine to a worker thread.
enum WorkerTask {
    /// Search this board position.
    Search(Board),
    /// Shut down permanently.
    Shutdown,
}

/// Sent from a worker thread back to SmpEngine after a search completes.
struct WorkerResult {
    nodes: u64,
}

/// Persistent thread pool for lazy SMP search.
///
/// Workers are spawned once at construction and live for the lifetime of the
/// engine. The SharedCache (DashMap) is also created once and reused across
/// all moves, accumulating the Transposition Table knowledge throughout the game.
pub struct SmpEngine {
    /// Shared all-way associative cache, reused across all solve() calls.
    shared: SharedCache,
    /// One sender per worker thread — used to dispatch tasks.
    worker_txs: Vec<mpsc::SyncSender<WorkerTask>>,
    /// Receiver for results from workers.
    result_rx: mpsc::Receiver<WorkerResult>,
    /// Stop flag shared with all workers — set when master finishes.
    stop: Arc<AtomicBool>,
}

impl SmpEngine {
    /// Create the engine and spawn all helper threads.
    pub fn new() -> Self {
        let shared: SharedCache = Arc::new(DashMap::new());
        let stop = Arc::new(AtomicBool::new(false));

        let (result_tx, result_rx) = mpsc::channel::<WorkerResult>();

        let mut worker_txs = Vec::with_capacity(HELPER_THREADS);

        for _ in 0..HELPER_THREADS {
            let (task_tx, task_rx) = mpsc::sync_channel::<WorkerTask>(1);
            worker_txs.push(task_tx);

            let shared  = Arc::clone(&shared);
            let stop    = Arc::clone(&stop);
            let result_tx = result_tx.clone();

            std::thread::spawn(move || {

                let mut transposition_table = ThreadLocalTranspositionTable {
                    shared,
                    array: ArrayCache::new(),
                    all_way_max_depth: TRANSPOSITION_TABLE_ALL_WAY_MAX_DEPTH,
                };

                loop {
                    match task_rx.recv() {
                        Err(_) | Ok(WorkerTask::Shutdown) => break,
                        Ok(WorkerTask::Search(board)) => {

                            let mut nodes = 0u64;
                            evaluate_position(
                                &board,
                                MIN_EVAL,
                                MAX_EVAL,
                                &mut transposition_table,
                                &mut nodes,
                                &stop
                            );

                            let _ = result_tx.send(WorkerResult { nodes });
                        }
                    }
                }
            });
        }//97912323

        SmpEngine {
            shared,
            worker_txs,
            result_rx,
            stop
        }
    }

    /// Solve a position. Master searches the full root on the calling thread;
    /// helpers search subtrees using the persistent shared cache.
    /// Returns (eval, node_counts) where index 0 = master.
    fn dispatch_helpers(&self, board: &Board) -> usize {
        self.stop.store(false, Ordering::Relaxed);

        let helper_starts = expand_starting_positions(board, HELPER_THREADS);
        let dispatched = helper_starts.len();

        for (task_tx, start) in self.worker_txs.iter().zip(helper_starts) {
            let _ = task_tx.send(WorkerTask::Search(start));
        }

        dispatched
    }

    fn collect_helpers(&self, dispatched: usize) {
        self.stop.store(true, Ordering::Relaxed);

        for _ in 0..dispatched {
            let _ = self.result_rx.recv();
        }
    }

    fn thread_local_transposition_table(&self) -> ThreadLocalTranspositionTable {
        ThreadLocalTranspositionTable {
            shared: Arc::clone(&self.shared),
            array: ArrayCache::new(),
            all_way_max_depth: TRANSPOSITION_TABLE_ALL_WAY_MAX_DEPTH,
        }
    }

    pub fn solve(&self, board: &Board) -> (i8, Vec<u64>) {
        let dispatched = self.dispatch_helpers(board);
        let mut transposition_table = self.thread_local_transposition_table();

        let mut master_nodes = 0u64;
        let stop_never = AtomicBool::new(false);
        let eval = evaluate_position(board, MIN_EVAL, MAX_EVAL, &mut transposition_table, &mut master_nodes, &stop_never);

        self.collect_helpers(dispatched);
        (eval, vec![master_nodes])
    }

    pub fn best_moves(&self, board: &Board) -> Vec<u32> {
        let dispatched = self.dispatch_helpers(board);
        let mut transposition_table = self.thread_local_transposition_table();

        let mut master_nodes = 0u64;
        let stop_never = AtomicBool::new(false);

        let mut iter = board.next_positions_with_col_ordered(&DEFAULT_MOVE_ORDER);
        let (first_col, first_next) = iter.next().expect("Board should not be full");

        let mut alpha = -evaluate_position(&first_next, -MAX_EVAL, -MIN_EVAL, &mut transposition_table, &mut master_nodes, &stop_never);
        let mut best_moves = vec![first_col];

        for (col, next) in iter {
            let null_eval = -evaluate_position(&next, -alpha, -alpha + 1, &mut transposition_table, &mut master_nodes, &stop_never);
            let eval = if null_eval >= alpha {
                -evaluate_position(&next, -MAX_EVAL, -null_eval, &mut transposition_table, &mut master_nodes, &stop_never)
            } else {
                null_eval
            };

            if eval > alpha {
                alpha = eval;
                best_moves = vec![col];
            } else if eval == alpha {
                best_moves.push(col);
            }
        }

        self.collect_helpers(dispatched);
        best_moves
    }
}

impl Drop for SmpEngine {
    fn drop(&mut self) {
        for tx in &self.worker_txs {
            let _ = tx.send(WorkerTask::Shutdown);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bitboard::Board;

    #[test]
    fn test_start_position() {
        let board = Board::from_str_engine_second(
            ". . . . . . .
             . . . . . . .
             . . . . . . .
             . . . X . . .
             . . . O . . .
             . O . X . . X"
        );

        board.display();

        let engine = SmpEngine::new();
        let (eval, node_counts) = engine.solve(&board);

        println!("Master nodes: {}", node_counts[0]);
        for (i, &nodes) in node_counts[1..].iter().enumerate() {
            println!("Helper {} nodes: {}", i + 1, nodes);
        }
        println!("Total nodes: {}", node_counts.iter().sum::<u64>());

        println!("Best Moves: {:?}", engine.best_moves(&board));

        assert_eq!(eval, 0, "Game should be a draw, got eval={}", eval);
    }
}
