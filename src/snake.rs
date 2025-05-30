// snakes moves from tail to head, eventually addding a piece on the tail if food is eated
// snake moves at 4 tiles per second
// and can only change direction towards its left or its right, other inputs are ignored
use std::time::{Duration, Instant};
use colored::Colorize;
use circular_buffer::CircularBuffer;
use std::collections::LinkedList;

const INIT_SNAKE_SIZE: i32 = 4;

use std::io;
use std::sync::mpsc;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::TryRecvError;
use std::thread;
extern crate num;
extern crate rand;
extern crate termios;
use std::io::Read;
use termios::{tcsetattr, Termios, ECHO, ICANON, TCSANOW};

type InputBuffer = CircularBuffer<1024, u8>; // 1024 bytes in input buffer

fn clear_screen() {
    print!("{}[2J", 27 as char);
    print!("{}[1;1H", 27 as char);
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SnakeDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GameDifficulty {
    Easy,
    Medium,
    Hard,
    Extreme,
    Impossible
}

impl GameDifficulty {
    pub fn get_speed(&self) -> u64 {
        match self {
            GameDifficulty::Easy => 500, // 2 fps
            GameDifficulty::Medium => 250, // 4 fps
            GameDifficulty::Hard => 166, // 6 fps
            GameDifficulty::Extreme => 125, // 8 fps
            GameDifficulty::Impossible => 100, // 10 fps
        }
    }
}

#[derive(Debug, Clone)]
pub struct Coordinates {
    x: i32,
    y: i32,
}

impl PartialEq for Coordinates {
    fn eq(&self, other: &Self) -> bool {
        self.x == other.x && self.y == other.y
    }
}

impl Coordinates {
    pub fn new(x: i32, y: i32) -> Coordinates {
        Coordinates { x, y }
    }

    pub fn move_left(&mut self) {
        self.x -= 1;
    }
    pub fn move_right(&mut self) {
        self.x += 1;
    }
    pub fn move_up(&mut self) {
        self.y -= 1;
    }
    pub fn move_down(&mut self) {
        self.y += 1;
    }
    pub fn get_left(&self) -> Coordinates {
        let mut res = Coordinates::new(self.x, self.y);
        res.move_left();
        res
    }
    pub fn get_right(&self) -> Coordinates {
        let mut res = Coordinates::new(self.x, self.y);
        res.move_right();
        res
    }
    pub fn get_up(&self) -> Coordinates {
        let mut res = Coordinates::new(self.x, self.y);
        res.move_up();
        res
    }
    pub fn get_down(&self) -> Coordinates {
        let mut res = Coordinates::new(self.x, self.y);
        res.move_down();
        res
    }
}

#[derive(Debug, Clone)]
pub struct SnakeGame {
    board_size: Coordinates,
    board: Vec<Vec<i32>>,
    snake_head_position: Coordinates,
    snake_body: LinkedList<Coordinates>, // The head is the first element
    snake_direction: SnakeDirection,
    food_position: Coordinates,
    points: i32,
    old_termios: Termios,
    new_termios: Termios,
    input_buffer: InputBuffer,
    difficulty: GameDifficulty,
}

impl SnakeGame {
    pub fn new(board_size: Coordinates) -> Self {
        Self::init_new(board_size, GameDifficulty::Medium)
    }

    pub fn new_with_difficulty(board_size: Coordinates, difficulty: GameDifficulty) -> Self {
        Self::init_new(board_size, difficulty)
    }

    fn init_new(board_size: Coordinates, difficulty: GameDifficulty) -> Self {
        // Initialize the game with a default board size
        if board_size.x < 10 || board_size.y < 10 {
            panic!("Board size must be at least 10x10");
        }
        let mut board = vec![vec![0; board_size.y as usize]; board_size.x as usize];
        let snake_head_position = Coordinates::new(6, 5);
        let mut snake_body: LinkedList<Coordinates> = LinkedList::new();
        let mut current_position = snake_head_position.clone();
        for _i in 0..INIT_SNAKE_SIZE {
            // 1 means occupied by snake
            board[current_position.x as usize][current_position.y as usize] = 1;
            snake_body.push_back(current_position.clone());
            current_position.move_left();
        }
        let half_way = (board_size.x + 6) / 2;
        board[half_way as usize][5] = 2; //initial Food position
        // setup terminal settings for non-blocking input
        let termios = Termios::from_fd(0).unwrap(); // 0 is file descriptor for stdin
        let mut new_termios = termios; // clone the termios struct
        new_termios.c_lflag &= !(ICANON | ECHO); // no echo and canonical mode for stdin
        SnakeGame {
            board_size,
            board,
            snake_head_position,
            snake_body,
            snake_direction: SnakeDirection::Right,
            food_position: Coordinates::new(half_way, 5),
            points: 0,
            old_termios: termios,
            new_termios,
            input_buffer: InputBuffer::new(),
            difficulty,
        }
    }

    pub fn play(&mut self) {
        // setup stdin to be non-blocking
        self.setup_streams();
        // spawn a thread to read from stdin
        let stdin_channel = self.spawn_stdin_channel();
        // main Game Loop happens here
        let mut frame_start_time = Instant::now();
        loop {
            let duration = frame_start_time.elapsed();
            // receive input from pipe
            match stdin_channel.try_recv() {
                Ok(key) => {
                    self.add_to_input_buffer(key);
                }
                Err(TryRecvError::Empty) => {}
                Err(TryRecvError::Disconnected) => panic!("Channel disconnected"),
            }
            if duration.as_millis() < self.difficulty.get_speed() as u128 {
                // wait for next frame
                std::thread::sleep(Duration::from_millis(3));
                continue;
            }
            // reset start
            frame_start_time = Instant::now();
            // get direction input
            let direction_to_move = self.get_direction_input();
            // apply action
            let old_tail_position = self.move_snake(direction_to_move);
            // try to eat food
            self.try_eating(old_tail_position);
            // check if the game is over due to the result of the action
            if self.is_over() {
                // println!("Game Over!");
                break;
            }
            // display the board
            clear_screen();
            self.display_board();
        }
        // reset the streams
        self.reset_streams();
        // When its over display final screen
        self.display_final_screen();
    }

    fn display_final_screen(&self) {
        // display the final screen
        println!("Game Over!");
        println!("Final Score: {}", self.points);
        // debug data (remove in the future)
        // println!("Snake head position: {:?}", self.snake_head_position);
        // println!("Snake body: {:?}", self.snake_body);
        // println!("Food position: {:?}", self.food_position);
    }

    fn is_over(&self) -> bool {
        // check if the snake head is out of bounds (snake hit the wall)
        if !self.is_in_bound(&self.snake_head_position) {
            return true;
        }
        // check if the snake head coords also appear in the body (snake bites itself)
        // iter over the snake skipping the head, which is always at the front
        for body_part in self.snake_body.iter().skip(1) {
            if self.snake_head_position == *body_part {
                return true;
            }
        }
        // check if the whole board is snake (game win)
        if self.points == (self.board_size.x * self.board_size.y) - INIT_SNAKE_SIZE {
            return true;
        }
        false
    }

    fn display_board(&self) {
        //border up
        print!("▗");
        for _i in 0..self.board_size.x {
            print!("▄▄");
        }
        // print a single element to end the border
        println!("▖");
        for i in 0..self.board_size.y {
            //border left
            print!("▐");
            // row of board
            for j in 0..self.board_size.x {
                let item = self.board[j as usize][i as usize];
                match item {
                    0 => print!("  "), // empty space
                    1 => { // snake body
                        if Coordinates::new(j,i) == self.snake_head_position {
                            // snake head
                            print!("{}","Ӫ ".yellow());
                        } else {
                            // snake body
                            print!("{}","⏺ ".green());
                        }
                    },
                    2 => print!("{}","♦ ".red()), // food
                    _ => panic!("Invalid item on the board!"),
                }
            }
            //border right
            print!("▌");
            //newline
            println!();
        }
        //border down
        print!("▝");
        for _i in 0..self.board_size.x {
            print!("▀▀");
        }
        print!("▘");
        println!();
        // score
        println!("Points: {}", self.points);
        // debug info (disable on release)
        // println!("Snake head position: {:?}", self.snake_head_position);
        // println!("Snake body: {:?}", self.snake_body);
        // println!("Food position: {:?}", self.food_position);
        // println!("Input buffer: {:?}", self.input_buffer);
    }

    fn is_moving_horizontally(&self) -> bool {
        matches!(
            self.snake_direction,
            SnakeDirection::Left | SnakeDirection::Right
        )
    }

    fn is_moving_vertically(&self) -> bool {
        matches!(
            self.snake_direction,
            SnakeDirection::Up | SnakeDirection::Down
        )
    }

    fn is_in_bound(&self, position: &Coordinates) -> bool {
        position.x >= 0 && position.x < self.board_size.x && position.y >= 0 && position.y < self.board_size.y
    }

    fn get_direction_input(&mut self) -> SnakeDirection {
        let mut result = self.snake_direction.clone();
        // arrows keys are long 3 bytes, first 2 need to be 27 and 91
        while self.input_buffer.len() >= 3 {
            // find the first arrow key in the buffer
            let mut found = false;
            if *self.input_buffer.nth_front(0).unwrap() == 27 &&
               *self.input_buffer.nth_front(1).unwrap() == 91 {
                // check the third byte for the arrow key
                match self.input_buffer.nth_front(2).unwrap() {
                    65 => {
                        // up arrow
                        if self.is_moving_horizontally() {
                            result = SnakeDirection::Up;
                            found = true;
                        }
                    }
                    66 => {
                        // down arrow
                        if self.is_moving_horizontally() {
                            result = SnakeDirection::Down;
                            found = true;
                        }
                    }
                    67 => {
                        // right arrow
                        if self.is_moving_vertically(){
                            result = SnakeDirection::Right;
                            found = true;
                        }
                    }
                    68 => {
                        // left arrow
                        if self.is_moving_vertically() {
                            result = SnakeDirection::Left;
                            found = true;
                        }
                    }
                    _ => {} // not an arrow
                }
            }
            // remove head of the buffer
            self.input_buffer.pop_front();
            // check if input was found
            if found {
                break;
            }
        }
        // clear the input buffer
        // self.input_buffer.clear();
        result
    }

    fn spawn_stdin_channel(&self) -> Receiver<u8> {
        let (tx, rx) = mpsc::channel::<u8>();
        thread::spawn(move || loop {
            // read one u8 at a time from the input buffer
            let mut reader = io::stdin();
            let mut buffer: [u8; 1] = [1; 1];
            reader.read_exact(&mut buffer).unwrap();
            tx.send(buffer[0]).unwrap();
        });
        rx
    }

    fn setup_streams(&mut self) {
        // setup stdin to not require enter press and not showing the input
        tcsetattr(0, TCSANOW, &self.new_termios).unwrap();
    }

    fn reset_streams(&mut self) {
        // reset stdin to default
        tcsetattr(0, TCSANOW, &self.old_termios).unwrap();
    }

    fn add_to_input_buffer(&mut self, key: u8) {
        if key == 1 {
            //1 is the default value and should be ignored
            return;
        }
        match key {
            27 | 91 | 65 | 66 | 67 | 68 => {
                // if the buffer is full, ignore the input
                if self.input_buffer.is_full(){
                    return;
                }
                self.input_buffer.push_back(key);
            } 
            _ => {
                // if it is not an arrow key, ignore the input
            }
        }
    }

    fn move_snake(&mut self, direction: SnakeDirection) -> Coordinates {
        // returns the old tail position
        self.snake_direction = direction.clone();
        let new_head = match direction {
            SnakeDirection::Up => self.snake_head_position.get_up(),
            SnakeDirection::Down => self.snake_head_position.get_down(),
            SnakeDirection::Left => self.snake_head_position.get_left(),
            SnakeDirection::Right => self.snake_head_position.get_right(),
        };

        // update tail first otherwise head may be overwritten by a 0
        // when the head of the snake is right behinf its tail
        let old_tail = self.snake_body.pop_back().unwrap();
        self.board[old_tail.x as usize][old_tail.y as usize] = 0;

        // update the head position
        self.snake_head_position = new_head.clone();
        // add the new head to the front of the snake body
        self.snake_body.push_front(new_head.clone());
        // update the board only if the head did not bump into a wall
        if self.is_in_bound(&new_head){
            // if the new head position is valid, update the board for new head,
            // otherwise the game will exit when checking for game over
            self.board[new_head.x as usize][new_head.y as usize] = 1;
        }

        // return the old tail position
        old_tail
    }

    fn try_eating(&mut self, old_tail: Coordinates) {
        if self.snake_head_position == self.food_position {
            //the food is eaten
            // increment score
            self.points += 1;
            // add the tail back to the snake
            self.snake_body.push_back(old_tail.clone());
            self.board[old_tail.x as usize][old_tail.y as usize] = 1;
            
            // generate new food
            if !self.is_over() {
                self.generate_food();
            }
        }
    }

    fn generate_food(&mut self) {
        let board_size = self.board_size.x * self.board_size.y;
        let snake_length = self.snake_body.len() as i32;
        if self.is_over() {
            //make sure that I am able to gen food
            return;
        }

        let food_position: Coordinates;
        // if snake occupies more than 80% of the board
        // choose a random elem in a vec of empty board positions,
        // else generate a random position until you find a free space
        if snake_length > (board_size * 4) / 5 {
            // use vec
            // collect all empty positions in the board
            let mut empty_positions: Vec<Coordinates> = vec![];
            for i in 0..self.board_size.x {
                for j in 0..self.board_size.y {
                    if self.board[i as usize][j as usize] == 0 {
                        empty_positions.push(Coordinates::new(i, j));
                    }
                }
            }
            // choose a random position from the empty positions
            // I know this is not perfectly identically distributed, but it is good enough for this game
            let random_index = num::abs(rand::random::<i32>()) % (empty_positions.len() as i32);

            food_position = empty_positions[random_index as usize].clone();
        } else {
            // use random choice
            loop {
                // generate a random position
                let random_x = num::abs(rand::random::<i32>()) % self.board_size.x;
                let random_y = num::abs(rand::random::<i32>()) % self.board_size.y;
                // check if the position is empty
                if self.board[random_x as usize][random_y as usize] == 0 {
                    // if it is empty, add food to the board
                    // and break the loop
                    food_position = Coordinates::new(random_x, random_y);
                    break;
                }
            }
        }
        // add food to the board
        self.add_food(food_position);
    }

    fn add_food(&mut self, position: Coordinates) {
        // add food to the board
        self.food_position = position.clone();
        self.board[position.x as usize][position.y as usize] = 2;
    }
}
