use std::io::{self, Write};
use crate::bitboard::Board;
use crate::smp_engine::SmpEngine;

mod bitboard;
mod smp_engine;

fn read_line() -> String {
    let mut input = String::new();
    io::stdin().read_line(&mut input).unwrap();
    input.trim().to_string()
}

fn main() {
    let engine = SmpEngine::new();

    println!("Queuenect 4 - pieces are inserted from the BOTTOM and push existing pieces up.");

    let engine_is_first = loop {
        print!("Go first or second? (1/2): ");
        io::stdout().flush().unwrap();
        match read_line().as_str() {
            "1" => break false,
            "2" => break true,
            _ => println!("Enter 1 or 2."),
        }
    };

    let mut board = Board::starting_board(engine_is_first);

    let human_piece = if engine_is_first { "O" } else { "X" };
    let engine_piece = if engine_is_first { "X" } else { "O" };
    println!("You are {}, engine is {}.\n", human_piece, engine_piece);

    loop {
        board.display();

        let curr_conn4 = board.current_player_connect4();
        let last_conn4 = board.last_player_connect4();

        let last_was_human = if engine_is_first {
            (board.moves_played & 1) == 1
        } else {
            (board.moves_played & 1) == 0
        };

        if curr_conn4 && last_conn4 {
            println!("Draw - both players have 4 in a row!");
            break;
        }
        if last_conn4 {
            if last_was_human {
                println!("You win!");
            } else {
                println!("Engine wins!");
            }
            break;
        }
        if curr_conn4 {
            if last_was_human {
                println!("You lose - you gave the engine 4 in a row!");
            } else {
                println!("You win - the engine gave you 4 in a row!");
            }
            break;
        }
        if board.moves_played == 42 {
            println!("Draw - board is full!");
            break;
        }

        let human_turn = engine_is_first == ((board.moves_played & 1) == 1);

        if human_turn {
            let col = loop {
                print!("Your move (0-6): {:?}", engine.best_moves(&board));
                io::stdout().flush().unwrap();

                match read_line().parse::<u32>() {
                    Ok(c) if c < 7 => {
                        if board.is_col_full(c) {
                            println!("Column {} is full, choose another.", c);
                        } else {
                            break c;
                        }
                    }
                    _ => println!("Invalid input, enter a number 0-6."),
                }
            };

            board.play(col);
        } else {
            print!("Engine thinking...");
            io::stdout().flush().unwrap();

            let moves = engine.best_moves(&board);
            let col = moves[rand::random_range(0..moves.len())];

            println!(" plays column {}", col);
            println!("Best Moves: {:?}", moves);
            board.play(col);
        }
    }
}
