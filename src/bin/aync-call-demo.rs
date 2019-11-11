#![feature(in_band_lifetimes)]

use std::fmt;

struct Node<'a> {
    children: Vec<Box<dyn fmt::Display + 'a>>,
}

impl<'a> Node<'a> {
    fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }
    fn add(mut self, child: impl fmt::Display + 'a) -> Self {
        self.children.push(Box::new(child));
        self
    }
}

impl fmt::Display for Node<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        for n in &self.children {
            write!(f, "{} ", n)?;
        }
        write!(f, "}}")
    }
}

fn main() {
    let tree = Node::new().add(1).add("foo").add(Node::new().add(2).add(3));
    println!("{}", tree);
}
