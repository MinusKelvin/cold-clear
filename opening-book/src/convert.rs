use opening_book::Book;

fn main() {
    let p = std::env::args().nth(1).unwrap();
    let f = std::fs::File::open(&p).unwrap();
    let b = Book::load(std::io::BufReader::new(f)).unwrap();
    let f = std::fs::File::create(".tmp.ccbook").unwrap();
    b.save(std::io::BufWriter::new(f)).unwrap();
    std::fs::rename(".tmp.ccbook", &p).unwrap();
}