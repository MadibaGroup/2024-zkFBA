use zeequent_lib::{Book, clearing, digest, Receipt};

// Number of price ticks.
global N: u32 = 1000;

// Outputs the digest of the book and its clearing receipt.
fn main(book: Book<N>, salt: Field) -> pub (Field, Receipt) {
    (digest(book, salt), clearing(book))
}