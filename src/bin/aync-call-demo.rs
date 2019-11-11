use async_call::{register, Registration};
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

struct Foo {
    _reg: Registration,
}

impl Foo {
    fn new() -> Self {
        Self { _reg: register() }
    }
}

impl fmt::Display for Foo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FOO")
    }
}

struct Bar {
    _reg: Registration,
}

impl Bar {
    fn new() -> Self {
        Self { _reg: register() }
    }
}

impl fmt::Display for Bar {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BAR")
    }
}

fn main() {
    let tree = Node::new()
        .add(1)
        .add(Foo::new())
        .add(Node::new().add(2).add(Bar::new()));
    println!("{}", tree);
}
