#![allow(dead_code)]

use snake::Coordinates;

mod snake;

fn main() {
    let mut game = snake::SnakeGame::new(Coordinates::new(20,12));
    game.play();
}
