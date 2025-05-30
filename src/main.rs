#![allow(dead_code)]

use snake::Coordinates;

mod snake;

fn main() {
    let board_size = Coordinates::new(20, 12);
    let mut game =
        snake::SnakeGame::new_with_difficulty(board_size, snake::GameDifficulty::Impossible);
    game.play();
}
