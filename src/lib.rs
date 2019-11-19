#[macro_use]
extern crate lazy_static;

use std::any::Any;
use std::collections::{HashMap, VecDeque};
use std::future::Future;
use std::hash::Hash;
use std::pin::Pin;
use std::sync::Mutex;
use std::task::{Context, Poll, Waker};

#[derive(Copy, Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct SrvId(usize);

impl SrvId {
    fn next(&self) -> Self {
        Self { 0: self.0 + 1 }
    }
}

#[derive(Copy, Clone, Default, PartialEq, Eq, Hash, Debug)]
pub struct ReqId(usize);

impl ReqId {
    fn next(&self) -> Self {
        Self { 0: self.0 + 1 }
    }
}

#[derive(Debug)]
struct MessageQueues {
    requests: HashMap<SrvId, VecDeque<(ReqId, Box<dyn Any + Send>)>>,
    responses: HashMap<ReqId, Option<Box<dyn Any + Send>>>,
    wakers: HashMap<ReqId, Waker>,
    next_srv_id: SrvId,
    next_req_id: ReqId,
}

impl MessageQueues {
    fn new() -> Self {
        Self {
            requests: HashMap::new(),
            responses: HashMap::new(),
            wakers: HashMap::new(),
            next_srv_id: SrvId::default(),
            next_req_id: ReqId::default(),
        }
    }
    fn register(&mut self) -> SrvId {
        let id = self.next_srv_id;
        self.requests.insert(id, VecDeque::new());
        self.next_srv_id = self.next_srv_id.next();
        id
    }
    fn unregister(&mut self, srv_id: SrvId) {
        self.requests.remove(&srv_id);
    }
    fn post_request(&mut self, srv_id: SrvId, request: Box<dyn Any + Send>) -> Option<ReqId> {
        if let Some(queue) = self.requests.get_mut(&srv_id) {
            let req_id = self.next_req_id;
            queue.push_front((req_id, request));
            self.next_req_id = self.next_req_id.next();
            Some(req_id)
        } else {
            None
        }
    }
    fn take_request(&mut self, srv_id: SrvId) -> Option<(ReqId, Box<dyn Any + Send>)> {
        if let Some(queue) = self.requests.get_mut(&srv_id) {
            queue.pop_back()
        } else {
            panic!("No service with id {:?} found", srv_id)
        }
    }
    fn set_response(&mut self, req_id: ReqId, resp: Option<Box<dyn Any + Send>>) {
        self.responses.insert(req_id, resp);
        if let Some(waker) = self.wakers.remove(&req_id) {
            waker.wake()
        }
    }
    fn check_response(
        &mut self,
        srv_id: SrvId,
        req_id: ReqId,
        waker: Waker,
    ) -> Result<Option<Box<dyn Any + Send>>, ()> {
        match self.responses.remove(&req_id) {
            // Normal response
            Some(resp @ Some(_)) => Ok(resp),
            // Request handled by service but not recognized
            Some(None) => Err(()),
            None => {
                if self.requests.contains_key(&srv_id) {
                    // Response pending
                    self.wakers.insert(req_id, waker);
                    Ok(None)
                } else {
                    // No service to answer
                    Err(())
                }
            }
        }
    }
}

lazy_static! {
    static ref GLOBAL_MESSAGE_QUEUES: Mutex<MessageQueues> = Mutex::new(MessageQueues::new());
}

pub fn send_request(
    srv_id: SrvId,
    request: impl Any + Send,
) -> impl Future<Output = Result<Box<dyn Any + Send>, ()>> {
    Request::new(srv_id, post_request(srv_id, Box::new(request)))
}

pub fn serve_requests<F>(srv_id: SrvId, mut f: F)
where
    F: FnMut(Box<dyn Any + Send>) -> Option<Box<dyn Any + Send>>,
{
    while let Some((req_id, req)) = take_request(srv_id) {
        set_response(req_id, f(req))
    }
}

pub struct ServiceRegistration {
    srv_id: SrvId,
}

impl ServiceRegistration {
    pub fn id(&self) -> SrvId {
        self.srv_id
    }
}

impl Drop for ServiceRegistration {
    fn drop(&mut self) {
        let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
        global.unregister(self.srv_id)
    }
}

pub fn register_service() -> ServiceRegistration {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    ServiceRegistration {
        srv_id: global.register(),
    }
}

fn post_request(srv_id: SrvId, request: Box<dyn Any + Send>) -> Option<ReqId> {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    global.post_request(srv_id, request)
}

fn take_request(srv_id: SrvId) -> Option<(ReqId, Box<dyn Any + Send>)> {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    global.take_request(srv_id)
}

fn set_response(req_id: ReqId, resp: Option<Box<dyn Any + Send>>) {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    global.set_response(req_id, resp)
}

fn check_response(
    srv_id: SrvId,
    req_id: ReqId,
    waker: Waker,
) -> Result<Option<Box<dyn Any + Send>>, ()> {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    global.check_response(srv_id, req_id, waker)
}

struct Request {
    srv_id: SrvId,
    opt_req_id: Option<ReqId>,
}

impl Request {
    fn new(srv_id: SrvId, opt_req_id: Option<ReqId>) -> Self {
        Self { srv_id, opt_req_id }
    }
}

impl Future for Request {
    type Output = Result<Box<dyn Any + Send>, ()>;
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        if let Some(req_id) = self.opt_req_id {
            match check_response(self.srv_id, req_id, cx.waker().clone()) {
                Ok(Some(resp)) => Poll::Ready(Ok(resp)),
                Ok(None) => Poll::Pending,
                Err(_) => Poll::Ready(Err(())),
            }
        } else {
            Poll::Ready(Err(()))
        }
    }
}
