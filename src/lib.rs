#[macro_use]
extern crate lazy_static;

use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;

#[derive(Debug)]
struct MessageQueues {
    message_queues: HashMap<usize, VecDeque<Box<dyn Any + Send>>>,
    next_id: usize,
}

impl MessageQueues {
    fn new() -> Self {
        Self {
            message_queues: HashMap::new(),
            next_id: 0,
        }
    }
    fn register(&mut self) -> usize {
        let id = self.next_id;
        self.message_queues.insert(id, VecDeque::new());
        self.next_id += 1;
        dbg!(self);
        id
    }
    fn unregister(&mut self, id: usize) {
        self.message_queues.remove(&id);
        dbg!(self);
    }
}

lazy_static! {
    static ref GLOBAL_MESSAGE_QUEUES: Mutex<MessageQueues> = Mutex::new(MessageQueues::new());
}

pub struct Registration {
    id: usize,
}

impl Drop for Registration {
    fn drop(&mut self) {
        let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
        global.unregister(self.id)
    }
}

pub fn register() -> Registration {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    Registration {
        id: global.register(),
    }
}
