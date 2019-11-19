use async_call::{register_service, SrvId, ServiceRegistration, send_request, serve_requests};
use std::fmt;
use async_std::{task, future};
use std::io;
use std::time::Duration;
use std::thread::spawn;

trait Update {
    fn update(&mut self) {}
}

trait Node : Update + fmt::Display {}
impl<T> Node for T where T: Update + fmt::Display {}

impl Update for u64 {}

struct Parent<'a> {
    children: Vec<Box<dyn Node + 'a>>,
}

impl<'a> Parent<'a> {
    fn new() -> Self {
        Self {
            children: Vec::new(),
        }
    }
    fn add(mut self, child: impl Node + 'a) -> Self {
        self.children.push(Box::new(child));
        self
    }
}

impl fmt::Display for Parent<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{{")?;
        for n in &self.children {
            write!(f, "{} ", n)?;
        }
        write!(f, "}}")
    }
}

impl Update for Parent<'_> {
    fn update(&mut self) {
       for c in &mut self.children {
           c.update();
       }
    }
}

struct Foo {
    counter: usize,
    reg: ServiceRegistration,
}

#[derive(Copy, Clone)]
struct FooId(SrvId);

enum FooOp {
    Inc,
}

impl FooId {
    async fn inc(self) -> Result<(),()> {
        dbg!("inc start");
        send_request(self.0, FooOp::Inc).await?;
        dbg!("inc end");
        Ok(())
    }
}

impl Update for Foo {
    fn update(&mut self) {
        serve_requests(self.reg.id(), |req| {
            if let Ok(op) = req.downcast::<FooOp>() {
                match *op {
                    FooOp::Inc => {
                        dbg!("inc");
                        self.inc();
                        Some(Box::new(()))
                    },
                }
            } else {
                None
            }
        })
    }
}

impl Foo {
    fn new() -> Self {
        Self {
            counter: 0,
            reg: register_service(),
        }
    }
    fn inc(&mut self) {
        self.counter += 1;
        dbg!(self.counter);
    }
    fn id(&self) -> FooId {
        FooId(self.reg.id())
    }
}

impl fmt::Display for Foo {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "FOO({})", self.counter)
    }
}

fn main() {
    let foo1 = Foo::new();
    let foo2 = Foo::new();
    let pfoo1 = foo1.id();
    let mut tree = Parent::new()
        .add(1)
        .add(foo1)
        .add(Parent::new().add(2).add(foo2));
    println!("before {}", tree);

//    spawn(move || {
//        task::block_on(future::pending::<()>());
//    });

    loop {
        let mut command = String::new();
        if let Ok(_) = io::stdin().read_line(&mut command) {
            match command.trim_end() {
                "inc" => {task::spawn(pfoo1.inc());},
                "exit" => break,
                other => println!("Unknown command {}", other)
            }
//            dbg!("1");
            task::block_on(future::timeout(Duration::default(), future::pending::<()>()));
//            dbg!("2");
            tree.update();
//            dbg!("3");
            task::block_on(future::timeout(Duration::default(), future::pending::<()>()));
            println!("{}", tree);
        }
    }
    println!("after {}", tree);
}
