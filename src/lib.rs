#[macro_use]
extern crate lazy_static;

use std::any::{Any, TypeId};
use std::collections::{HashMap, VecDeque};
use std::fmt::Debug;
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

pub trait Message: Any + Debug + Send {}
impl<T> Message for T where T: Any + Debug + Send {}
impl dyn Message {
    // Code copied from impl dyn Any
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        if self.type_id() == TypeId::of::<T>() {
            unsafe { Some(&*(self as *const dyn Message as *const T)) }
        } else {
            None
        }
    }
}

#[derive(Debug)]
struct MessageQueues {
    requests: HashMap<SrvId, VecDeque<(ReqId, Box<dyn Message>)>>,
    responses: HashMap<ReqId, Option<Box<dyn Message>>>,
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
    fn dbg(&self) {
        for (srv_id, reqs) in &self.requests {
            if !reqs.is_empty() {
                println!("{:?} : {:?}", srv_id, reqs);
            }
        }
        for (req_id, resp) in &self.responses {
            if let Some(resp) = resp {
                println!("{:?} : {:?}", req_id, resp);
            }
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
    fn post_request(&mut self, srv_id: SrvId, request: Box<dyn Message>) -> Option<ReqId> {
        if let Some(queue) = self.requests.get_mut(&srv_id) {
            let req_id = self.next_req_id;
            queue.push_front((req_id, request));
            self.next_req_id = self.next_req_id.next();
            Some(req_id)
        } else {
            None
        }
    }
    fn take_request(&mut self, srv_id: SrvId) -> Option<(ReqId, Box<dyn Message>)> {
        if let Some(queue) = self.requests.get_mut(&srv_id) {
            queue.pop_back()
        } else {
            panic!("No service with id {:?} found", srv_id)
        }
    }
    fn set_response(&mut self, req_id: ReqId, resp: Option<Box<dyn Message>>) {
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
    ) -> Result<Option<Box<dyn Message>>, ()> {
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

pub fn dbg_message_queues() {
    let global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    global.dbg();
}

pub fn send_request(
    srv_id: SrvId,
    request: impl Message,
) -> impl Future<Output = Result<Box<dyn Message>, ()>> {
    Request::new(srv_id, post_request(srv_id, Box::new(request)))
}

pub async fn send_request_typed<T>(srv_id: SrvId, request: impl Message) -> Result<T, ()>
where
    T: Message + Copy,
{
    let answer = send_request(srv_id, request).await?;
    if let Some(res) = answer.downcast_ref::<T>() {
        Ok(*res)
    } else {
        Err(())
    }
}

pub fn serve_requests<F>(srv_id: SrvId, mut f: F)
where
    F: FnMut(Box<dyn Message>) -> Option<Box<dyn Message>>,
{
    while let Some((req_id, req)) = take_request(srv_id) {
        set_response(req_id, f(req))
    }
}

pub fn serve_requests_typed<T, F>(srv_id: SrvId, mut f: F)
where
    T: Any + Send + Copy,
    F: FnMut(T) -> Option<Box<dyn Message>>,
{
    serve_requests(srv_id, |req| {
        if let Some(op) = req.downcast_ref::<T>() {
            f(*op)
        } else {
            None
        }
    })
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

fn post_request(srv_id: SrvId, request: Box<dyn Message>) -> Option<ReqId> {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    global.post_request(srv_id, request)
}

fn take_request(srv_id: SrvId) -> Option<(ReqId, Box<dyn Message>)> {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    global.take_request(srv_id)
}

fn set_response(req_id: ReqId, resp: Option<Box<dyn Message>>) {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    global.set_response(req_id, resp)
}

fn check_response(
    srv_id: SrvId,
    req_id: ReqId,
    waker: Waker,
) -> Result<Option<Box<dyn Message>>, ()> {
    let mut global = GLOBAL_MESSAGE_QUEUES.lock().unwrap();
    global.check_response(srv_id, req_id, waker)
}

#[derive(Debug)]
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
    type Output = Result<Box<dyn Message>, ()>;
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
