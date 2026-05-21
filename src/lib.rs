/// Queuennect 4 Bitboard
///
/// Board layout: 7 columns x 6 rows, each column occupies 7 bits (6 piece bits + 1 sentinel)
/// Bit index for (row, col): col * 7 + row
/// Row 0 = bottom, Row 5 = top, Row 6 = sentinel (never holds a real piece)
///
///  col:   0      1      2      3      4      5      6
///  bits: 0-6   7-13  14-20  21-27  28-34  35-41  42-48
///
///  within each column:
///  bit 6 = sentinel
///  bit 5 = row 5 (top)
///  ...
///  bit 0 = row 0 (bottom)  <-- insertion point
///
/// Two bitboards: `current` (side to move) and `opponent`
/// Unique position key: current | ((current | opponent) + BOTTOM_MASK)

pub const ROWS: u32 = 6;
pub const COLS: u32 = 7;
pub const COL_STRIDE: u32 = 7; // bits per column (6 piece bits + 1 sentinel)

/// Bit 0 of each column — the insertion point for every push
pub const BOTTOM_MASK: u64 = {
    let mut mask = 0u64;
    let mut col = 0u32;
    while col < COLS {
        mask |= 1u64 << (col * COL_STRIDE);
        col += 1;
    }
    mask
};

/// Bit 5 of each column — the topmost valid piece row
pub const TOP_MASK: u64 = {
    let mut mask = 0u64;
    let mut col = 0u32;
    while col < COLS {
        mask |= 1u64 << (col * COL_STRIDE + ROWS - 1);
        col += 1;
    }
    mask
};

/// Bit 6 of each column — the sentinel row (never a real piece)
pub const SENTINEL_MASK: u64 = {
    let mut mask = 0u64;
    let mut col = 0u32;
    while col < COLS {
        mask |= 1u64 << (col * COL_STRIDE + ROWS);
        col += 1;
    }
    mask
};

/// All valid piece bits (bits 0-5 of every column), excludes sentinels
pub const BOARD_MASK: u64 = {
    let mut mask = 0u64;
    let mut col = 0u32;
    while col < COLS {
        let mut row = 0u32;
        while row < ROWS {
            mask |= 1u64 << (col * COL_STRIDE + row);
            row += 1;
        }
        col += 1;
    }
    mask
};

const MIN_EVAL: i32 = -18;
const MAX_EVAL: i32 = 18;

/// Returns the full column mask (6 piece bits) for a given column
#[inline]
pub const fn col_mask(col: u32) -> u64 {
    0x3F << (col * COL_STRIDE)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    /// Bitboard for the current player (side to move)
    pub current: u64,
    /// Bitboard for the opponent
    pub opponent: u64,
    /// Total number of moves played so far
    pub moves_played: u32,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MoveResult {
    Ok,
    ColumnFull,
}

impl Board {
    pub fn new() -> Self {
        Board { current: 0, opponent: 0, moves_played: 0 }
    }

    /// Returns true if column `col` is full (all 6 piece bits occupied)
    #[inline]
    pub fn is_col_full(&self, col: u32) -> bool {
        let occupied = self.current | self.opponent;
        (occupied >> (col * COL_STRIDE)) & 0x3F == 0x3F
    }

    /// Push a piece into the bottom of `col` for the current player,
    /// shifting all existing pieces in that column up by one.
    /// Returns ColumnFull if the column has no room.
    /// Switches side to move on success.
    pub fn play(&mut self, col: u32) -> MoveResult {
        if self.is_col_full(col) {
            return MoveResult::ColumnFull;
        }

        let shift = col * COL_STRIDE;
        let mask = col_mask(col);

        // Shift both players' pieces in this column up by 1
        self.current  = (self.current  & !mask) | ((self.current  & mask) << 1);
        self.opponent = (self.opponent & !mask) | ((self.opponent & mask) << 1);

        // Insert new piece at row 0 of this column
        self.current |= 1u64 << shift;

        // Sanity: sentinel bits must never be set
        debug_assert!(self.current  & SENTINEL_MASK == 0, "current player overflowed into sentinel");
        debug_assert!(self.opponent & SENTINEL_MASK == 0, "opponent overflowed into sentinel");

        self.moves_played += 1;

        // Switch sides
        std::mem::swap(&mut self.current, &mut self.opponent);

        MoveResult::Ok
    }

    /// Check if the last move (now stored in `opponent` since we swapped) was a win.
    /// Call this AFTER play() — the player who just moved is now `opponent`.
    pub fn is_win_for_last_player(&self) -> bool {
        Self::has_four(self.opponent)
    }

    /// Check if a given bitboard has 4 in a row in any direction
    fn has_four(board: u64) -> bool {
        // Horizontal: stride = 7 (one full column-slot)
        if Self::four_in_direction(board, COL_STRIDE) { return true; }
        // Vertical: stride = 1 (one row within a column)
        if Self::four_in_direction(board, 1) { return true; }
        // Diagonal /: stride = COL_STRIDE + 1
        if Self::four_in_direction(board, COL_STRIDE + 1) { return true; }
        // Diagonal \: stride = COL_STRIDE - 1
        if Self::four_in_direction(board, COL_STRIDE - 1) { return true; }
        false
    }

    /// Standard 4-in-a-row check via repeated AND+shift
    #[inline]
    fn four_in_direction(board: u64, stride: u32) -> bool {
        let m = board & (board >> stride);
        m & (m >> (2 * stride)) != 0
    }

    /// Unique position key for transposition table (Tromp-style)
    /// key = current | ((current | opponent) + BOTTOM_MASK)
    #[inline]
    pub fn key(&self) -> u64 {
        let occupied = self.current | self.opponent;
        self.current | (occupied.wrapping_add(BOTTOM_MASK))
    }

    /// Bitmask of playable columns (columns that are not full)
    #[inline]
    pub fn valid_moves_mask(&self) -> u64 {
        let mut mask = 0u64;
        for col in 0..COLS {
            if !self.is_col_full(col) {
                mask |= 1u64 << (col * COL_STRIDE);
            }
        }
        mask
    }

    /// List of playable column indices
    pub fn valid_moves(&self) -> Vec<u32> {
        (0..COLS).filter(|&c| !self.is_col_full(c)).collect()
    }

    /// Encode a board from a string for testing.
    ///
    /// The string must contain exactly 6 rows and 7 columns of cells separated
    /// by whitespace. Row order is top-to-bottom (first row = row 5, last = row 0).
    /// Valid cell characters: 'X', 'O', '.' (empty).
    ///
    /// Current player is inferred from piece counts:
    ///   - X moves first; if counts are equal it is X's turn.
    ///   - If O has exactly 1 fewer piece than X, it is O's turn.
    ///
    /// Panics (debug_assert) if counts differ by more than 1.
    pub fn from_str(s: &str) -> Self {
        let cells: Vec<char> = s
            .split_whitespace()
            .map(|tok| {
                let c = tok.chars().next().expect("empty token");
                debug_assert!(
                    matches!(c, 'X' | 'O' | '.'),
                    "invalid cell character '{}': must be X, O, or .",
                    c
                );
                c
            })
            .collect();

        debug_assert_eq!(
            cells.len(),
            (ROWS * COLS) as usize,
            "expected {} cells, got {}",
            ROWS * COLS,
            cells.len()
        );

        let x_count = cells.iter().filter(|&&c| c == 'X').count() as i32;
        let o_count = cells.iter().filter(|&&c| c == 'O').count() as i32;

        debug_assert!(
            (x_count - o_count).abs() <= 1,
            "piece counts differ by more than 1: X={} O={}",
            x_count,
            o_count
        );

        // X moves first; X is current when counts are equal.
        // O is current only when O has exactly 1 fewer piece than X (X just moved).
        let x_is_current = x_count == o_count;

        let mut x_bits = 0u64;
        let mut o_bits = 0u64;

        // cells are in top-to-bottom, left-to-right order
        // row index in string: 0 = top = row 5, 5 = bottom = row 0
        for (i, &c) in cells.iter().enumerate() {
            let string_row = (i as u32) / COLS;
            let col        = (i as u32) % COLS;
            let board_row  = (ROWS - 1) - string_row; // flip: top of string = row 5
            let bit        = col * COL_STRIDE + board_row;
            match c {
                'X' => x_bits |= 1u64 << bit,
                'O' => o_bits |= 1u64 << bit,
                _   => {}
            }
        }

        let (current, opponent) = if x_is_current {
            (x_bits, o_bits)
        } else {
            (o_bits, x_bits)
        };

        Board {
            current,
            opponent,
            moves_played: (x_count + o_count) as u32,
        }
    }

    pub fn display(&self) {
        let occupied = self.current | self.opponent;
        println!(" 0 1 2 3 4 5 6");
        for row in (0..ROWS).rev() {
            print!("|");
            for col in 0..COLS {
                let bit = col * COL_STRIDE + row;
                if (self.opponent >> bit) & 1 == 1 {
                    print!("X ");
                } else if (self.current >> bit) & 1 == 1 {
                    print!("O ");
                } else if (occupied >> bit) & 1 == 0 {
                    print!(". ");
                }
            }
            println!("|");
        }
        println!(" 0 1 2 3 4 5 6");
    }
}

impl Default for Board {
    fn default() -> Self {
        Self::new()
    }
}

pub const TOTAL_CELLS: u32 = ROWS * COLS; // 42

/// Win score = 22 - moves_played_by_winner.
#[inline]
fn win_score(moves_played: u32) -> i32 {
    22 - ((moves_played + 2) / 2) as i32
}

/// Default move ordering — centre-first.
pub const MOVE_ORDER: [u32; COLS as usize] = [3, 2, 4, 1, 5, 0, 6];

// ---------------------------------------------------------------------------
// Transposition table
// ---------------------------------------------------------------------------

pub const TT_SIZE: usize = 1 << 20; // 2^20 entries for the array cache
const TT_MASK: u64 = (TT_SIZE - 1) as u64;

/// Default moves_played threshold below which positions go into the HashMap.
/// Positions with moves_played <= this value are stored in the all-way associative
/// HashMap (no collisions, exact lookup). Deeper positions use the array.
pub const DEFAULT_EXACT_DEPTH: u32 = 18;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Bound {
    /// Score is a lower bound (came from a beta cutoff)
    Lower,
    /// Score is an upper bound (no cutoff was found)
    Upper,
}

#[derive(Clone, Copy)]
pub struct TtEntry {
    pub key:   u64,
    pub score: i8,
    pub bound: Bound,
}

/// Hybrid transposition table.
///
/// - Positions with `moves_played <= exact_depth` are stored in a `HashMap`
///   (all-way associative: no collisions, exact lookup). These are the most
///   expensive nodes — near the root of the search tree — so correctness matters.
/// - All other positions use a fixed-size array (one-way associative, may collide).
///   These are cheap to recompute and benefit more from raw lookup speed.
pub struct TranspositionTable {
    /// All-way associative cache for shallow (expensive) positions
    exact: std::collections::HashMap<u64, TtEntry>,
    /// One-way associative array cache for deeper (cheaper) positions
    array: Vec<TtEntry>,
    /// moves_played threshold: <= this goes into `exact`, > this goes into `array`
    pub exact_depth: u32,
}

impl TranspositionTable {
    pub fn new() -> Self {
        Self::with_exact_depth(DEFAULT_EXACT_DEPTH)
    }

    pub fn with_exact_depth(exact_depth: u32) -> Self {
        TranspositionTable {
            exact: std::collections::HashMap::new(),
            array: vec![TtEntry { key: u64::MAX, score: 0, bound: Bound::Lower }; TT_SIZE],
            exact_depth,
        }
    }

    #[inline]
    fn array_index(key: u64) -> usize {
        (key & TT_MASK) as usize
    }

    #[inline]
    pub fn store(&mut self, key: u64, score: i32, bound: Bound, moves_played: u32) {
        let entry = TtEntry { key, score: score as i8, bound };
        if moves_played <= self.exact_depth {
            self.exact.insert(key, entry);
        } else {
            let idx = Self::array_index(key);
            self.array[idx] = entry;
        }
    }

    #[inline]
    pub fn get(&self, key: u64, moves_played: u32) -> Option<TtEntry> {
        if moves_played <= self.exact_depth {
            self.exact.get(&key).copied()
        } else {
            let entry = self.array[Self::array_index(key)];
            if entry.key == key { Some(entry) } else { None }
        }
    }

    /// Number of entries currently stored in the exact (HashMap) cache
    pub fn exact_len(&self) -> usize {
        self.exact.len()
    }
}

impl Default for TranspositionTable {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Negamax
// ---------------------------------------------------------------------------

/// Negamax with a caller-supplied move ordering and transposition table.
/// `nodes` is incremented once per call (i.e. per position visited).
pub fn negamax_ordered(
    board: &Board,
    mut alpha: i32,
    mut beta: i32,
    order: &[u32],
    tt: &mut TranspositionTable,
    nodes: &mut u64,
) -> i32 {
    *nodes += 1;

    // Draw: board completely full, no winner found above us
    if board.moves_played == TOTAL_CELLS {
        return 0;
    }

    // Check for an immediate win before recursing.
    for &col in order {
        if board.is_col_full(col) {
            continue;
        }
        let mut next = *board;
        next.play(col);
        if next.is_win_for_last_player() {
            return win_score(board.moves_played);
        }
    }

    // Tighten beta with the theoretical score ceiling
    beta = beta.min(win_score(board.moves_played + 1));
    if alpha >= beta {
        return beta;
    }

    // Transposition table lookup
    let key = board.key();
    if let Some(entry) = tt.get(key, board.moves_played) {
        match entry.bound {
            Bound::Lower => alpha = alpha.max(entry.score as i32),
            Bound::Upper => beta  = beta.min(entry.score as i32),
        }
        if alpha >= beta {
            return entry.score as i32;
        }
    }

    // Recurse over all moves that don't win immediately
    for &col in order {
        if board.is_col_full(col) {
            continue;
        }
        let mut next = *board;
        next.play(col);
        if next.is_win_for_last_player() {
            continue;
        }

        let score = -negamax_ordered(&next, -beta, -alpha, order, tt, nodes);
        if score >= beta {
            tt.store(key, score, Bound::Lower, board.moves_played);
            return score;
        }
        if score > alpha {
            alpha = score;
        }
    }

    tt.store(key, alpha, Bound::Upper, board.moves_played);
    alpha
}

/// Negamax with alpha-beta pruning and a transposition table.
pub fn negamax(board: &Board, alpha: i32, beta: i32, tt: &mut TranspositionTable, nodes: &mut u64) -> i32 {
    negamax_ordered(board, alpha, beta, &MOVE_ORDER, tt, nodes)
}

/// Solve a position, returning the minimax score and the number of nodes searched.
pub fn solve(board: &Board, tt: &mut TranspositionTable) -> (i32, u64) {
    let mut nodes = 0u64;
    let score = negamax(board, MIN_EVAL, MAX_EVAL, tt, &mut nodes);
    (score, nodes)
}

/// Solve with a custom move ordering, returning score and node count.
pub fn solve_ordered(board: &Board, order: &[u32], tt: &mut TranspositionTable) -> (i32, u64) {
    let mut nodes = 0u64;
    let score = negamax_ordered(board, MIN_EVAL, MAX_EVAL, order, tt, &mut nodes);
    (score, nodes)
}

/// Convenience: create a fresh TT and solve, returning score and node count.
pub fn fresh_solve(board: &Board) -> (i32, u64) {
    let mut tt = TranspositionTable::new();
    solve(board, &mut tt)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_start_position() {
        let board = Board::from_str(
            ". . . . . . .
             . . . . . . .
             . . . . . . .
             . . . . . . .
             . . . . . . .
             . . . . . . ."
        );
        let (score, pos) = fresh_solve(&board);

        println!("Positions: {}", pos);

        assert_eq!(score, -9, "X should be in a losing position, got score={}", score);
    }
}
