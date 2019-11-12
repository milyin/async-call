use async_call::{register, Id, Registration};
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
    counter: usize,
    reg: Registration,
}

#[derive(Copy, Clone)]
struct FooId(Id);

enum FooOp {
    Inc,
}

impl FooId {
    async fn inc(&self) -> Result<(),()> {
        self.0.send_request(FooOp::Inc).await?;
        Ok(())
    }
}

impl Foo {
    fn new() -> Self {
        Self {
            counter: 0,
            reg: register(),
        }
    }
    fn inc(&mut self) {
        self.counter += 1
    }
    fn id(&self) -> FooId {
        FooId(self.reg.id())
    }
    fn update(&self) {
        self.reg.process_requests(|req| None)
    }
}

impl fmt::Display for Foo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FOO")
    }
}

fn main() {
    let foo1 = Foo::new();
    let foo2 = Foo::new();
    let pfoo1 = foo1.id();
    let tree = Node::new()
        .add(1)
        .add(foo1)
        .add(Node::new().add(2).add(foo2));
    println!("{}", tree);
    let future = pfoo1.inc();
    // How to run it?
    println!("{}", tree);
}
