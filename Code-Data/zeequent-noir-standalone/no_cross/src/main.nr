use zeequent_lib::{Book, check_no_cross, digest};

// Number of price ticks.
global N: u32 = 1000;

// Outputs the digest of a book that does not cross.
fn main(book: Book<N>, salt: Field) -> pub Field {
    check_no_cross(book);
    digest(book, salt)
}