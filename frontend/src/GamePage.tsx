import { Stack, Group, Button, Text, Box, ScrollArea, Flex, Divider } from '@mantine/core';
import { Board } from './Board';
import { StatusBar } from './StatusBar';
import {type MoveRecord, useGameState} from './useGameState';

export function GamePage({onNavigateToSandbox, onNavigateToAnalyze}: {
    onNavigateToSandbox: () => void,
    onNavigateToAnalyze: (history: MoveRecord[]) => void
}) {
    const { grid, status, playerOrder, winningCells, moveHistory, startGame, resetGame, handleColumnClick } = useGameState();

    const boardDisabled = status !== 'player-turn' || playerOrder === null;
    const isGameInProgress = playerOrder !== null && (status === 'player-turn' || status === 'engine-turn');

    const pairedMoves = [];
    for (let i = 0; i < moveHistory.length; i += 2) {
        pairedMoves.push({
            turn: Math.floor(i / 2) + 1,
            first: moveHistory[i],
            second: moveHistory[i + 1],
        });
    }

    return (
        <Box
            style={{
                minHeight: '100vh',
                backgroundColor: '#151c28',
                display: 'flex',
                alignItems: 'center',
                justifyContent: 'center',
                padding: '2rem',
            }}
        >
            <Stack align="center" gap="xl">
                <Stack align="center" gap={4}>
                    <Text
                        style={{
                            fontSize: 28,
                            fontWeight: 500,
                            color: '#e8edf5',
                            letterSpacing: '0.02em',
                        }}
                    >
                        Queuennect 4
                    </Text>
                </Stack>

                <StatusBar status={status} playerOrder={playerOrder} />

                <Flex
                    direction={{ base: 'column', md: 'row' }}
                    align="center"
                    gap="xl"
                >
                    <Box w={180} display={{ base: 'none', md: 'block' }} />

                    <Box style={{ maxWidth: '100vw', overflowX: 'auto', padding: '10px 0' }}>
                        <Board
                            grid={grid}
                            disabled={boardDisabled}
                            onColumnClick={handleColumnClick}
                            winningCells={winningCells}
                        />
                    </Box>

                    <Box
                        w={{ base: '100%', md: 180 }}
                        style={{
                            height: 470,
                            maxWidth: 400,
                            backgroundColor: '#1c2535',
                            border: '0.5px solid #252f42',
                            borderRadius: 16,
                            display: 'flex',
                            flexDirection: 'column',
                        }}
                    >
                        <Text size="sm" fw={500} c="#e8edf5" p="md" style={{ borderBottom: '1px solid #252f42' }}>
                            Move History
                        </Text>

                        <ScrollArea style={{ flex: 1, height: 410 }} p="md" offsetScrollbars>
                            <Stack gap="xs">
                                {pairedMoves.map((pair, index) => (
                                    <Group key={index} wrap="nowrap" gap="sm">
                                        <Text size="xs" c="#6b7a99" style={{ minWidth: 20 }}>{pair.turn}.</Text>

                                        <Text size="sm" fw={500} style={{ width: 24, textAlign: 'center' }} c={pair.first.piece === 'red' ? '#ef4444' : '#facc15'}>
                                            {String.fromCharCode(65 + pair.first.col)}
                                        </Text>

                                        {pair.second && (
                                            <Text size="sm" fw={500} style={{ width: 24, textAlign: 'center' }} c={pair.second.piece === 'red' ? '#ef4444' : '#facc15'}>
                                                {String.fromCharCode(65 + pair.second.col)}
                                            </Text>
                                        )}
                                    </Group>
                                ))}
                            </Stack>
                        </ScrollArea>
                    </Box>
                </Flex>

                <Stack align="center" gap="md" mt="sm">

                    {isGameInProgress ? (
                        <Button
                            variant="outline"
                            color="red"
                            size="sm"
                            onClick={resetGame}
                        >
                            Rage quit
                        </Button>
                    ) : (
                        <Stack align="center" gap="xs">
                            <Text size="xs" fw={600} c="#6b7a99" style={{ textTransform: 'uppercase' }}>
                                Start New Game
                            </Text>
                            <Group>
                                <Button
                                    color="red"
                                    onClick={() => startGame('first')}
                                    styles={{ root: { backgroundColor: '#ef4444', color: '#ffffff' } }}
                                >
                                    Play First (Red)
                                </Button>
                                <Button
                                    color="yellow"
                                    onClick={() => startGame('second')}
                                    styles={{ root: { backgroundColor: '#facc15', color: '#000000' } }}
                                >
                                    Play Second (Yellow)
                                </Button>
                            </Group>

                            {moveHistory.length > 0 && (
                                <Button
                                    mt="sm"
                                    color="indigo"
                                    onClick={() => onNavigateToAnalyze(moveHistory)}
                                >
                                    Analyze Game
                                </Button>
                            )}
                        </Stack>
                    )}

                    <Box style={{ width: '100%', maxWidth: 300, padding: '12px 0' }}>
                        <Divider color="#252f42" />
                    </Box>

                    <Button
                        variant="outline"
                        color="gray"
                        size="md"
                        onClick={onNavigateToSandbox}
                        styles={{
                            root: {
                                borderColor: '#4a5568',
                                color: '#e8edf5',
                                '&:hover': { backgroundColor: '#1c2535' }
                            }
                        }}
                    >
                        Enter Engine Sandbox
                    </Button>
                </Stack>
            </Stack>
        </Box>
    );
}