use async_call::{register_service, SrvId, ServiceRegistration, send_request, serve_requests_typed, send_request_typed};
use std::fmt;
use async_std::{task, future};
use std::time::Duration;
use std::thread::{spawn, sleep};
use std::rc::Rc;

trait Update {
    fn update(&mut self) {}
}

trait Node : Update + fmt::Display {}
impl<T> Node for T where T: Update + fmt::Display {}

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

struct Value {
    value: usize,
    reg: ServiceRegistration,
}

#[derive(Copy, Clone)]
struct ValueId(SrvId);

enum ValueOp {
    Set(usize),
    Get,
}

impl ValueId {
    async fn set(self, value: usize) -> Result<(),()> {
        send_request(self.0, ValueOp::Set(value)).await?;
        Ok(())
    }
    async fn get(self) -> Result<usize,()> {
        send_request_typed(self.0, ValueOp::Get).await
    }
}

impl Update for Value {
    fn update(&mut self) {
        serve_requests_typed(self.reg.id(), |req| {
                match req {
                    ValueOp::Get => {
                        Some(Box::new(self.get()))
                    },
                    ValueOp::Set(value) => {
                        self.set(value);
                        Some(Box::new(()))
                    }
                }
        })
    }
}

impl Value {
    fn new() -> Self {
        Self {
            value: 0,
            reg: register_service(),
        }
    }
    fn get(&self) -> usize {
        self.value
    }
    fn set(&mut self, value: usize) {
        self.value = value
    }
    fn id(&self) -> ValueId {
        ValueId(self.reg.id())
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VALUE({})", self.value)
    }
}

struct Button<'a> {
    on_click: Option<Rc<dyn Fn() + 'a>>,
    reg: ServiceRegistration,
}

impl<'a> Button<'a> {
    pub fn new() -> Self {
        Self { on_click: None, reg: register_service() }
    }
    pub fn on_click(&mut self, handler: impl Fn() + 'a) {
        self.on_click = Some(Rc::new(move || { handler(); }));
    }
}

enum ButtonOp {
    Click,
}

impl<'a> Update for Button<'a> {
    fn update(&mut self) {
        serve_requests_typed(self.reg.id(), |req| {
                match req {
                    ButtonOp::Click => {
                        dbg!("click");
                        Some(Box::new(()))
                    },
                }
        })
    }
}

fn main() {
    let foo1 = Value::new();
    let foo2 = Value::new();
    let pfoo1 = foo1.id();
    let pfoo2 = foo1.id();
    let mut tree = Parent::new()
        .add(foo1)
        .add(Parent::new().add(foo2));
    println!("before {}", tree);

    spawn( move || {
        task::block_on(future::pending::<()>());
    });

    task::spawn(async move {
        pfoo1.set(42).await.unwrap();
        let q = pfoo1.get().await.unwrap();
        pfoo2.set(q+1).await.unwrap();
    } );

    for step in 0..10 {
        println!("{} : {}", step, tree);
        tree.update();
        sleep(Duration::from_secs(1));
    }

}
