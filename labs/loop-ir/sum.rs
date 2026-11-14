#[inline(never)]
fn next(value: i64) -> i64 {
    value + 1
}

#[inline(never)]
fn sum_next(limit: i64) -> i64 {
    let mut index = 0;
    let mut total = 0;
    while index < limit {
        total += next(index);
        index += 1;
    }
    total
}

fn main() {
    let limit: i64 = std::env::args()
        .nth(1)
        .expect("Supply a limit.")
        .parse()
        .expect("The limit must be a signed 64-bit integer.");
    println!("{}", sum_next(limit));
}
