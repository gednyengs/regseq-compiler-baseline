#![no_std]
#![feature(register_tool)]
#![register_tool(rseq)]

pub struct Point {
    x: i32,
    y: i32,
}

pub fn add(p1: &Point, p2: &Point) -> (i32, i32) {
    let sum_x = p1.x + p2.x;
    let sum_y = p1.y + p2.y;
    (sum_x, sum_y)
}
