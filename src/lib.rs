#[macro_use]
extern crate lazy_static;

use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll};

#[derive(Debug)]
struct MessageQueues {
    requests: HashMap<usize, VecDeque<(usize, Box<dyn Any + Send>)>>,
    responses: HashMap<(usize, usize), Option<Box<dyn Any + Send>>>,
    next_obj_id: usize,
    next_req_id: usize,
}

impl MessageQueues {
    fn new() -> Self {
        Self {
            requests: HashMap::new(),
            responses: HashMap::new(),
            next_obj_id: 0,
            next_req_id: 0,
        }
    }
    fn register(&mut self) -> usize {
        let id = self.next_obj_id;
        self.requests.insert(id, VecDeque::new());
        self.next_obj_id += 1;
        dbg!(self);
        id
    }
    fn unregister(&mut self, id: usize) {
        self.requests.remove(&id);
        dbg!(self);
    }
    fn post_request(&mut self, id: usize, request: Box<dyn Any + Send>) -> Option<usize> {
        if let Some(queue) = self.requests.get_mut(&id) {
            let id = self.next_req_id;
            queue.push_front((id, request));
            self.next_req_id += 1;
            dbg!(self);
            Some(id)
        } else {
            dbg!(self);
            None
        }
    }
    fn take_request(&mut self, id: usize) -> Option<(usize, Box<dyn Any + Send>)> {
        if let Some(queue) = self.requests.get_mut(&id) {
            queue.pop_back()
        } else {
            panic!("No client with id {} found", id)
        }
    }
    fn set_response(&mut self, id: usize, req_id: usize, resp: Option<Box<dyn Any + Send>>) {
        self.responses.insert((id, req_id), resp);
    }
    fn check_response(
        &mut self,
        id: usize,
        req_id: usize,
    ) -> Result<Option<Box<dyn Any + Send>>, ()> {
        match self.responses.remove(&(id, req_id)) {
            // Normal response
            Some(resp @ Some(_)) => Ok(resp),
            // Request not recognized
            Some(None) => Err(()),
            None => {
                if self.requests.contains_key(&id) {
                    // Response pending
                    Ok(None)
                } else {
                    // No one to answer
                    Err(())
                }
            }
        }
    }
}

lazy_static! {
    static ref GLOBAL_MESSAGE_QUEUES: Mutex<MessageQueues> = Mutex::new(MessageQueues::new());
}

#[derive(Copy, Clone)]
pub struct Id(usize);

impl Id {
    pub fn send_request(
        &self,
        request: impl Any + Send,
    ) -> impl Future<Output = Result<Box<dyn Any + Send>, ()>> {
        Request::new(self.0, post_request(self.0, Box::new(request)))
    }
}

pub struct Registration {
    id: usize,
}

impl Registration {
    pub fn id(&self) -> Id {
        Id(self.id)
    }

    pub fn process_requests<F>(&self, f: F)
    where
        F: Fn(Box<dyn Any + Send>) -> Option<Box<dyn Any + Send>>,
    {
        while let Some((req_id, req)) = take_request(self.id) {
            set_response(self.id, req_id, f(req))
        }
    }
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

fn post_request(id: usize, request: Box<dyn Any + Send>) -> Option<usize> {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    global.post_request(id, request)
}

fn take_request(id: usize) -> Option<(usize, Box<dyn Any + Send>)> {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    global.take_request(id)
}

fn set_response(id: usize, req_id: usize, resp: Option<Box<dyn Any + Send>>) {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    global.set_response(id, req_id, resp)
}

fn check_response(id: usize, req_id: usize) -> Result<Option<Box<dyn Any + Send>>, ()> {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    global.check_response(id, req_id)
}

struct Request {
    id: usize,
    req_id: Option<usize>,
}

impl Request {
    fn new(id: usize, req_id: Option<usize>) -> Self {
        Self { id, req_id }
    }
}

impl Future for Request {
    type Output = Result<Box<dyn Any + Send>, ()>;
    fn poll(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(req_id) = self.req_id {
            match check_response(self.id, req_id) {
                Ok(Some(resp)) => Poll::Ready(Ok(resp)),
                Ok(None) => Poll::Pending,
                Err(_) => Poll::Ready(Err(())),
            }
        } else {
            Poll::Ready(Err(()))
        }
    }
}
