use actix_web::{web, App, HttpServer, HttpResponse, middleware};
use actix_cors::Cors;
use serde::{Deserialize, Serialize};
use std::sync::Mutex;

mod bitboard;
mod smp_engine;

use bitboard::{Board, TOTAL_CELLS};
use smp_engine::SmpEngine;

struct AppState {
    engine: Mutex<SmpEngine>,
}

/// 42-character flat board string.
/// Characters 0–6 = row 0 (bottom), 35–41 = row 5 (top).
/// 'X' = engine, 'O' = human, '.' = empty.
#[derive(Deserialize)]
struct BoardRequest {
    board: String,
}

#[derive(Serialize)]
struct BestMovesResponse {
    best_moves: Vec<u32>,
    /// Eval from the current player's perspective (positive = winning).
    eval: i8,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

/// POST /api/best_moves
///
/// Body: { "board": ".............................................."}
/// Returns: { "best_moves": [3, 4], "eval": 12 }
async fn best_moves(
    state: web::Data<AppState>,
    body: web::Json<BoardRequest>,
) -> HttpResponse {
    let board = match Board::from_str_flat(&body.board) {
        Ok(b)  => b,
        Err(e) => return HttpResponse::BadRequest().json(ErrorResponse { error: e }),
    };

    if board.moves_played == TOTAL_CELLS {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "Board is already full".into(),
        });
    }
    if board.last_player_connect4() || board.current_player_connect4() {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "Game is already over".into(),
        });
    }

    board.display();

    let engine = state.engine.lock().unwrap();
    let (eval, _nodes) = engine.solve(&board);
    let best_moves = engine.best_moves(&board);

    HttpResponse::Ok().json(BestMovesResponse { best_moves, eval })
}

async fn evaluate_moves(
    state: web::Data<AppState>,
    body: web::Json<BoardRequest>,
) -> HttpResponse {
    println!("Evaluate Moves called");

    let board = match Board::from_str_flat(&body.board) {
        Ok(b)  => b,
        Err(e) => return HttpResponse::BadRequest().json(ErrorResponse { error: e }),
    };

    if board.moves_played == TOTAL_CELLS {
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "Board is already full".into(),
        });
    }
    if board.last_player_connect4() || board.current_player_connect4() {
        println!("Game Already Over");
        return HttpResponse::BadRequest().json(ErrorResponse {
            error: "Game is already over".into(),
        });
    }

    let engine = state.engine.lock().unwrap();
    let mut evals: Vec<Option<i8>> = vec![None; 7];

    board.display();

    for (col, next) in board.next_positions_with_col() {
        next.display();
        let (next_eval, _nodes) = engine.solve(&next);
        evals[col as usize] = Some(-next_eval);
    }

    HttpResponse::Ok().json(evals)
}

/// GET /api/health
async fn health() -> HttpResponse {
    HttpResponse::Ok().json(serde_json::json!({ "status": "ok" }))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    env_logger::init_from_env(env_logger::Env::default().default_filter_or("info"));

    let data = web::Data::new(AppState {
        engine: Mutex::new(SmpEngine::new()),
    });

    println!("Listening on http://127.0.0.1:8080");

    HttpServer::new(move || {
        let cors = Cors::default()
            .allow_any_origin()
            .allow_any_method()
            .allow_any_header();

        App::new()
            .wrap(cors)
            .wrap(middleware::Logger::default())
            .app_data(data.clone())
            .route("/api/health",     web::get().to(health))
            .route("/api/best_moves", web::post().to(best_moves))
            .route("/api/evaluate_moves", web::post().to(evaluate_moves))
    })
        .bind("127.0.0.1:8080")?
        .run()
        .await
}
