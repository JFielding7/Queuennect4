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
pub const TOTAL_CELLS: u32 = ROWS * COLS; // 42
const COL_STRIDE: u32 = 7; // bits per column (6 piece bits + 1 sentinel)

/// Bit 0 of each column — the insertion point for every push
const BOTTOM_MASK: u64 = {
    let mut mask = 0u64;
    let mut col = 0u32;
    while col < COLS {
        mask |= 1u64 << (col * COL_STRIDE);
        col += 1;
    }
    mask
};

/// Bit 6 of each column — the sentinel row (never a real piece)
const SENTINEL_MASK: u64 = {
    let mut mask = 0u64;
    let mut col = 0u32;
    while col < COLS {
        mask |= 1u64 << (col * COL_STRIDE + ROWS);
        col += 1;
    }
    mask
};

const SINGLE_COL_MASK: u64 = 0x3F;

const SINGLE_COL_MASK_WITH_SENTINEL: u64 = 0x7F;

/// Returns the full column mask (6 piece bits) for a given column
#[inline]
pub const fn col_mask(col: u32) -> u64 {
    SINGLE_COL_MASK << (col * COL_STRIDE)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Board {
    /// Bitboard for the current player (side to move)
    pub current: u64,
    /// Bitboard for the opponent
    pub opponent: u64,
    /// Total number of moves played so far
    pub moves_played: u32,
    /// If Engine goes first
    pub is_engine_first: bool,
}

#[derive(Debug, PartialEq, Eq)]
pub enum MoveResult {
    Ok,
    ColumnFull,
}

impl Board {
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

    /// Standard 4-in-a-row check via repeated AND+shift
    #[inline]
    fn four_in_direction(board: u64, stride: u32) -> bool {
        let m = board & (board >> stride);
        m & (m >> (2 * stride)) != 0
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

    pub fn current_player_connect4(&self) -> bool {
        Self::has_four(self.current)
    }

    /// Check if the last move (now stored in `opponent` since we swapped) was a win.
    /// Call this AFTER play() — the player who just moved is now `opponent`.
    pub fn last_player_connect4(&self) -> bool {
        Self::has_four(self.opponent)
    }


    /// Unique position key for transposition table (Tromp-style)
    /// key = current | ((current | opponent) + BOTTOM_MASK)
    #[inline]
    fn normal_key(&self) -> u64 {
        let occupied = self.current | self.opponent;
        self.current | (occupied + BOTTOM_MASK)
    }

    #[inline]
    fn reverse_key(key: u64) -> u64 {
        let mut reversed = 0u64;

        for col in 0..COLS {
            let mirrored_col = COLS - 1 - col;
            let src_shift = col * COL_STRIDE;
            let dst_shift = mirrored_col * COL_STRIDE;
            reversed |= ((key >> src_shift) & SINGLE_COL_MASK_WITH_SENTINEL) << dst_shift;
        }

        reversed
    }

    pub fn key(&self) -> u64 {
        let normal_key = self.normal_key();
        normal_key.min(Self::reverse_key(normal_key))
    }

    pub fn next_positions_ordered<'a>(&'a self, order: &'a [u32]) -> impl Iterator<Item = Board> + 'a {
        order.iter().filter_map(|&col| {
            if self.is_col_full(col) {
                return None;
            }
            let mut next = *self;
            next.play(col);
            Some(next)
        })
    }

    pub fn next_positions_with_col_ordered<'a>(&'a self, order: &'a [u32]) -> impl Iterator<Item = (u32, Board)> + 'a {
        order.iter().filter_map(|&col| {
            if self.is_col_full(col) {
                return None;
            }
            let mut next = *self;
            next.play(col);
            Some((col, next))
        })
    }

    pub fn starting_board(is_engine_first: bool) -> Board {
        Board::from_str(
            ". . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . .",
            is_engine_first
        )
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
    pub fn from_str(s: &str, is_engine_first: bool) -> Board {
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
            is_engine_first,
        }
    }

    pub fn from_str_engine_first(s: &str) -> Self {
        Self::from_str(s, true)
    }

    pub fn from_str_engine_second(s: &str) -> Self {
        Self::from_str(s, false)
    }

    pub fn display(&self) {
        let (x_bits, o_bits) = if self.moves_played % 2 == 0 {
            // X is current (even moves played, X moves next)
            (self.current, self.opponent)
        } else {
            // X is opponent (odd moves played, O moves next)
            (self.opponent, self.current)
        };

        println!("  0 1 2 3 4 5 6");
        for row in (0..ROWS).rev() {
            print!("| ");
            for col in 0..COLS {
                let bit = col * COL_STRIDE + row;
                if (x_bits >> bit) & 1 == 1 {
                    print!("X ");
                } else if (o_bits >> bit) & 1 == 1 {
                    print!("O ");
                } else {
                    print!(". ");
                }
            }
            println!("|");
        }
        println!("  0 1 2 3 4 5 6");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_board_no_win() {
        let board = Board::starting_board(true);

        assert!(!board.current_player_connect4());
        assert!(!board.last_player_connect4());
    }

    #[test]
    fn test_column_fills_and_blocks() {
        let mut board = Board::starting_board(true);

        for _ in 0..6 {
            assert_eq!(board.play(0), MoveResult::Ok);
        }

        assert!(board.is_col_full(0));
        assert_eq!(board.play(0), MoveResult::ColumnFull);
    }

    #[test]
    fn test_vertical_win() {
        let mut board = Board::starting_board(true);

        for _ in 0..3 {
            assert_eq!(board.play(0), MoveResult::Ok);
            assert_eq!(board.play(1), MoveResult::Ok);
        }

        assert_eq!(board.play(0), MoveResult::Ok);
        assert!(board.last_player_connect4(), "Expected vertical win");
    }

    #[test]
    fn test_horizontal_win() {
        let mut board = Board::starting_board(true);
        for col in 0..3 {
            assert_eq!(board.play(col), MoveResult::Ok);
            assert_eq!(board.play(6), MoveResult::Ok);
        }
        assert_eq!(board.play(3), MoveResult::Ok);
        assert!(board.last_player_connect4(), "Expected horizontal win");
    }

    #[test]
    fn test_key_uniqueness_simple() {
        let mut b0 = Board::starting_board(true);
        let mut b1 = Board::starting_board(true);
        b0.play(0);
        b1.play(1);
        assert_ne!(b0.key(), b1.key());
    }

    #[test]
    fn test_from_str_empty_board() {
        let board = Board::from_str_engine_first(
            "
            . . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . ."
        );

        assert_eq!(board.current,      0);
        assert_eq!(board.opponent,     0);
        assert_eq!(board.moves_played, 0);
    }

    #[test]
    fn test_from_str_x_moves_first_equal_counts() {
        let board = Board::from_str_engine_first(
            "
            . . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . .
            X . . . O . ."
        );

        assert_eq!(board.moves_played, 2);

        let x_bit = 0 * COL_STRIDE + 0;
        assert_eq!(board.current  & (1 << x_bit), 1 << x_bit, "X should be current");

        let o_bit = 4 * COL_STRIDE + 0;
        assert_eq!(board.opponent & (1 << o_bit), 1 << o_bit, "O should be opponent");
    }

    #[test]
    fn test_from_str_o_moves_next_when_one_fewer() {
        let board = Board::from_str_engine_first(
            "
            . . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . .
            X X . . O . ."
        );

        assert_eq!(board.moves_played, 3);

        let o_bit = 4 * COL_STRIDE + 0;
        assert_eq!(board.current & (1 << o_bit), 1 << o_bit, "O should be current");
    }

    #[test]
    fn test_from_str_row_ordering() {
        let board = Board::from_str_engine_first(
            "
            X . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . ."
        );

        let top_bit = 0 * COL_STRIDE + 5;
        assert_eq!(board.opponent & (1 << top_bit), 1 << top_bit, "X should be at row 5 in opponent");

        let bot_bit = 0 * COL_STRIDE + 0;
        assert_eq!(board.opponent & (1 << bot_bit), 0, "row 0 should be empty");
        assert_eq!(board.current  & (1 << bot_bit), 0, "row 0 should be empty");
    }

    #[test]
    fn test_from_str_vertical_win_detected() {
        let board = Board::from_str_engine_first(
            "
            . . . . . . .
            . . . . . . .
            X . . . . . .
            X . . . . . O
            X . . . . . O
            X . . . . . O"
        );
        assert!(board.last_player_connect4(), "X should have 4 in a column");
    }

    #[test]
    fn test_moves_played_counter() {
        let mut board = Board::starting_board(true);
        assert_eq!(board.moves_played, 0);

        board.play(0);
        assert_eq!(board.moves_played, 1);

        board.play(1);
        assert_eq!(board.moves_played, 2);
    }

    #[test]
    fn test_simultaneous_connect4() {
        let board = Board::from_str_engine_first(
            "
            . . . . . . .
            . . . . . . .
            . . . . . . .
            . . . . . . .
            . X X X . . .
            . O O O X . ."
        );

        let mut board = board;
        board.play(4);

        board.display();

        assert!(board.current_player_connect4() && board.last_player_connect4(),
                "Expected simultaneous connect4");
    }

    #[test]
    fn test_diagonal_win_forward() {
        let board = Board::from_str_engine_first(
            "
            . . . . . . .
            . . . . . . .
            . . . X . . .
            . . X O . . .
            . X O O . . .
            X O O X . . ."
        );

        assert!(board.current_player_connect4(), "Expected forward diagonal win for X");
    }

    #[test]
    fn test_diagonal_win_backward() {
        let board = Board::from_str_engine_first(
            "
            . . . . . . .
            . . . . . . .
            . . . X . . .
            . . X O . . .
            . X O O . . .
            X O X O . . ."
        );

        assert!(board.current_player_connect4(), "Expected backward diagonal win for X");
    }

    #[test]
    fn test_key_symmetric_positions_equal() {
        let mut b0 = Board::starting_board(true);
        let mut b1 = Board::starting_board(true);
        b0.play(0);
        b1.play(6);
        assert_eq!(b0.key(), b1.key(), "Mirrored positions should share a key");
    }

    #[test]
    fn test_full_board_no_win_is_not_win() {
        let board = Board::from_str_engine_first(
            "
            X X O O X X O
            O O X X O O X
            X X O O X X O
            O O X X O O X
            X X O O X X O
            O O X X O O X"
        );

        assert!(!board.current_player_connect4());
        assert!(!board.last_player_connect4());
        assert_eq!(board.moves_played, 42);
    }

    #[test]
    fn test_push_shifts_pieces_up() {
        let mut board = Board::starting_board(true);
        board.play(0);
        board.play(1);
        board.play(0);

        let row1_bit = 0 * COL_STRIDE + 1;
        assert_eq!(board.opponent & (1 << row1_bit), 1 << row1_bit,
                   "X should have been pushed to row 1");
    }

    #[test]
    fn test_no_valid_moves_when_full() {
        let mut board = Board::starting_board(true);

        for _ in 0..6 {
            for col in 0..7u32 {
                board.play(col);
            }
        }
        assert!(board.next_positions_ordered(&[0, 1, 2, 3, 4, 5, 6]).next().is_none(),
                "No moves should be available on a full board");
    }

    #[test]
    fn test_sentinel_never_set_after_many_moves() {
        let sentinel_mask: u64 = (0..7u32).fold(0, |acc, col| acc | (1u64 << (col * 7 + 6)));
        let mut board = Board::starting_board(true);

        for _ in 0..5 {
            for col in 0..7u32 {
                board.play(col);
            }
        }

        assert_eq!(board.current & sentinel_mask, 0);
        assert_eq!(board.opponent & sentinel_mask, 0);
    }
}
