# async-call

Consider typical GUI implementation where widgets are grouped into some tree.

Usually it's OK for any widget to call methods of other widgets. For example button can have a handler which adds row to table. Yes, in large projects it may lead to a mess and better to avoid such architectures. But on the other hand sometimes it can be convenient.

In Rust it's impossible to call one widget from another because of borrow checker. All widgets belongs to their hosts which holds mutable references to them. So only parent is allowed to do access it's childs.

This restriction can be bypassed using messages. Node A can't access node B directly, but it can post some message for B and wait for answer.

The purpose of this library is to wrap this message passing into async method calls to make user code clear.

This is how GUI code made with help of async-call library may look:

    let button_obj = Button::new();
    let counter_obj = Counter::new();
    let button = button_obj.id();
    let counter = counter_obj.id();

    button_obj.on_click(|| {
       let v = counter.get_value();
       counter.set_value(v + 1);
    });

    let dialog = Dialog::new()
      .add(text_obj)
      .add(button_obj);

Here get_value() and set_value() are async methods which internally post messages to global queue and waits for answer.
