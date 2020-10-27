use opening_book::Book;

fn main() {
    for p in std::env::args().skip(1) {
        println!("Converting {}...", p);
        let f = std::fs::File::open(&p).unwrap();
        let b = Book::load(std::io::BufReader::new(f)).unwrap();
        let f = std::fs::File::create(format!("{}.done", p)).unwrap();
        b.save(std::io::BufWriter::new(f)).unwrap();
    }
}
