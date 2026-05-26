import { useState, useCallback } from 'react';
import type { CellState } from './Cell';
import type { GameStatus } from './StatusBar';

export const COLS = 7;
export const ROWS = 6;

export interface MoveRecord {
    by: 'player' | 'engine';
    piece: 'red' | 'yellow';
    col: number;
}

export function emptyGrid(): CellState[][] {
    return Array.from({ length: COLS }, () => Array<CellState>(ROWS).fill('empty'));
}

export function gridToFlatString(grid: CellState[][]): string {
    let s = '';
    for (let row = 0; row < ROWS; row++) {
        for (let col = 0; col < COLS; col++) {
            const cell = grid[col][row];

            if (cell === 'red') s += 'X';
            else if (cell === 'yellow') s += 'O';
            else s += '.';
        }
    }
    return s;
}

export function checkWin(grid: CellState[][], piece: CellState): { col: number, row: number }[] | null {
    for (let row = 0; row < ROWS; row++) {
        for (let col = 0; col <= COLS - 4; col++) {
            if (grid[col][row] === piece && grid[col+1][row] === piece &&
                grid[col+2][row] === piece && grid[col+3][row] === piece)
                return [{col, row}, {col: col+1, row}, {col: col+2, row}, {col: col+3, row}];
        }
    }
    for (let col = 0; col < COLS; col++) {
        for (let row = 0; row <= ROWS - 4; row++) {
            if (grid[col][row] === piece && grid[col][row+1] === piece &&
                grid[col][row+2] === piece && grid[col][row+3] === piece)
                return [{col, row}, {col, row: row+1}, {col, row: row+2}, {col, row: row+3}];
        }
    }
    for (let col = 0; col <= COLS - 4; col++) {
        for (let row = 0; row <= ROWS - 4; row++) {
            if (grid[col][row] === piece && grid[col+1][row+1] === piece &&
                grid[col+2][row+2] === piece && grid[col+3][row+3] === piece)
                return [{col, row}, {col: col+1, row: row+1}, {col: col+2, row: row+2}, {col: col+3, row: row+3}];
        }
    }
    for (let col = 0; col <= COLS - 4; col++) {
        for (let row = 3; row < ROWS; row++) {
            if (grid[col][row] === piece && grid[col+1][row-1] === piece &&
                grid[col+2][row-2] === piece && grid[col+3][row-3] === piece)
                return [{col, row}, {col: col+1, row: row-1}, {col: col+2, row: row-2}, {col: col+3, row: row-3}];
        }
    }
    return null;
}

export function playPiece(grid: CellState[][], col: number, piece: CellState): CellState[][] | null {
    if (grid[col][ROWS - 1] !== 'empty') return null;
    const next = grid.map(c => [...c]);
    for (let row = ROWS - 2; row >= 0; row--) {
        if (next[col][row] !== 'empty' && (row == 0 || next[col][row - 1] !== 'empty')) {
            next[col][row + 1] = next[col][row];
        }
    }
    next[col][0] = piece;
    return next;
}

export function isBoardFull(grid: CellState[][]): boolean {
    return grid.every(col => col[ROWS - 1] !== 'empty');
}

async function fetchBestMoves(grid: CellState[][]): Promise<number[]> {
    const board = gridToFlatString(grid);
    const res = await fetch('http://localhost:8080/api/best_moves', {
        method: 'POST',
        headers: { 'Content-Type': 'application/json' },
        body: JSON.stringify({ board }),
    });
    if (!res.ok) throw new Error(`API error: ${res.status}`);
    const data = await res.json();
    return data.best_moves as number[];
}

export type PlayerOrder = 'first' | 'second';

export interface GameState {
    grid: CellState[][];
    status: GameStatus;
    playerOrder: PlayerOrder | null;
    winningCells: { col: number, row: number }[] | null;
    moveHistory: MoveRecord[];
    startGame: (order: PlayerOrder) => void;
    resetGame: () => void;
    handleColumnClick: (col: number) => void;
}

export function useGameState(): GameState {
    const [grid, setGrid] = useState<CellState[][]>(emptyGrid);
    const [status, setStatus] = useState<GameStatus>('player-turn');
    const [playerOrder, setPlayerOrder] = useState<PlayerOrder | null>(null);
    const [winningCells, setWinningCells] = useState<{ col: number, row: number }[] | null>(null);
    const [moveHistory, setMoveHistory] = useState<MoveRecord[]>([]);

    const runEngineMove = useCallback(async (currentGrid: CellState[][], enginePiece: 'red' | 'yellow', humanPiece: 'red' | 'yellow') => {
        setStatus('engine-turn');

        try {
            const moves = await fetchBestMoves(currentGrid);
            const col = moves[Math.floor(Math.random() * moves.length)];

            const next = playPiece(currentGrid, col, enginePiece);
            if (!next) return;

            const engineWins = checkWin(next, enginePiece);
            const playerWins = checkWin(next, humanPiece);

            setGrid(next);
            setMoveHistory(prev => [...prev, { by: 'engine', piece: enginePiece, col }]);

            if (engineWins && playerWins) {
                setStatus('draw');
            } else if (engineWins) {
                setStatus('engine-win');
                setWinningCells(engineWins);
            } else if (playerWins) {
                setStatus('player-win');
                setWinningCells(playerWins);
            } else if (isBoardFull(next)) {
                setStatus('draw');
            } else {
                setStatus('player-turn');
            }
        } catch (e) {
            console.error('Engine move failed:', e);
        }
    }, []);

    const startGame = useCallback(async (order: PlayerOrder) => {
        const fresh = emptyGrid();
        setGrid(fresh);
        setPlayerOrder(order);
        setWinningCells(null);
        setMoveHistory([]);

        const ePiece = order === 'first' ? 'yellow' : 'red';
        const hPiece = order === 'first' ? 'red' : 'yellow';

        if (order === 'second') {
            await runEngineMove(fresh, ePiece, hPiece);
        } else {
            setStatus('player-turn');
        }
    }, [runEngineMove]);

    const resetGame = useCallback(() => {
        setGrid(emptyGrid());
        setPlayerOrder(null);
        setWinningCells(null);
        setMoveHistory([]);
        setStatus('player-turn');
    }, []);

    const handleColumnClick = useCallback(async (col: number) => {
        if (status !== 'player-turn' || playerOrder === null) return;

        const hPiece = playerOrder === 'first' ? 'red' : 'yellow';
        const ePiece = playerOrder === 'first' ? 'yellow' : 'red';

        const next = playPiece(grid, col, hPiece);
        if (!next) return;

        const playerWins = checkWin(next, hPiece);
        const engineWins = checkWin(next, ePiece);

        setGrid(next);
        setMoveHistory(prev => [...prev, { by: 'player', piece: hPiece, col }]);

        if (playerWins && engineWins) { setStatus('draw'); return; }
        if (playerWins) {
            setStatus('player-win');
            setWinningCells(playerWins);
            return;
        }
        if (engineWins) {
            setStatus('engine-win');
            setWinningCells(engineWins);
            return;
        }
        if (isBoardFull(next)) { setStatus('draw'); return; }

        await runEngineMove(next, ePiece, hPiece);
    }, [grid, status, playerOrder, runEngineMove]);

    return { grid, status, playerOrder, winningCells, moveHistory, startGame, resetGame, handleColumnClick };
}