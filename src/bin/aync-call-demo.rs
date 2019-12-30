#![feature(async_closure)]

use async_call::{
    dbg_message_queues, register_service, send_request, send_request_typed, serve_requests_typed,
    ServiceRegistration, SrvId,
};
use async_std::{future, task};
use std::fmt;
use std::future::Future;
use std::rc::Rc;
use std::thread::{sleep, spawn};
use std::time::Duration;

trait Update {
    fn update(&mut self) {}
}

trait Node: Update + fmt::Display {}
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

#[derive(Copy, Clone, Debug)]
enum ValueOp {
    Set(usize),
    Get,
}

impl ValueId {
    async fn set(self, value: usize) -> Result<(), ()> {
        send_request(self.0, ValueOp::Set(value)).await?;
        Ok(())
    }
    async fn get(self) -> Result<usize, ()> {
        send_request_typed(self.0, ValueOp::Get).await
    }
}

impl Update for Value {
    fn update(&mut self) {
        serve_requests_typed(self.reg.id(), |req| match req {
            ValueOp::Get => Some(Box::new(self.get())),
            ValueOp::Set(value) => {
                self.set(value);
                Some(Box::new(()))
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

#[derive(Copy, Clone)]
struct ButtonId(SrvId);

impl ButtonId {
    async fn click(&self) -> Result<(), ()> {
        send_request_typed(self.0, ButtonOp::Click).await
    }
}

struct Button<'a> {
    on_click: Option<Rc<dyn Fn() + 'a>>,
    reg: ServiceRegistration,
}

impl<'a> Button<'a> {
    pub fn new() -> Self {
        Self {
            on_click: None,
            reg: register_service(),
        }
    }
    pub fn on_click<FUTURE: 'static + Send + Future<Output = ()>, T: 'a + Fn() -> FUTURE>(
        &mut self,
        handler: T,
    ) {
        self.on_click = Some(Rc::new(move || {
            task::spawn(handler());
        }));
    }
    fn id(&self) -> ButtonId {
        ButtonId(self.reg.id())
    }
}

impl<'a> fmt::Display for Button<'a> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "VALUE")
    }
}

#[derive(Copy, Clone, Debug)]
enum ButtonOp {
    Click,
}

impl<'a> Update for Button<'a> {
    fn update(&mut self) {
        serve_requests_typed(self.reg.id(), |req| match req {
            ButtonOp::Click => {
                dbg!("Click");
                if let Some(ref handler) = self.on_click {
                    handler();
                }
                Some(Box::new(()))
            }
        })
    }
}

fn main() {
    let valA = Value::new();
    let valB = Value::new();
    let valAB = Value::new();
    let mut button = Button::new();
    let pvalA = valA.id();
    let pvalB = valB.id();
    let pvalAB = valAB.id();
    let pbtn = button.id();
    button.on_click(async move || {
        dbg!("on click");
        let a = pvalA.get().await.unwrap();
        let b = pvalB.get().await.unwrap();
        pvalAB.set(a + b).await;
    });
    let mut tree = Parent::new()
        .add(valA)
        .add(valB)
        .add(Parent::new().add(valAB).add(button));
    println!("before {}", tree);

    spawn(move || {
        task::block_on(future::pending::<()>());
    });

    task::spawn(async move {
        dbg!("set a");
        pvalA.set(42).await.unwrap();
        dbg!("get a");
        let q = pvalA.get().await.unwrap();
        dbg!("set b");
        pvalB.set(q + 1).await.unwrap();
        dbg!("click");
        pbtn.click().await;
        dbg!("end");
    });

    for step in 0..10 {
        println!("{} : {}", step, tree);
        dbg_message_queues();
        tree.update();
        dbg_message_queues();
        sleep(Duration::from_secs(1));
    }
}
