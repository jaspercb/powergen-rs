// Strum contains all the trait definitions
extern crate strum;
#[macro_use]
extern crate strum_macros;

use std::cell::RefCell;
use std::sync::{Arc, Weak};
use std::any::Any;

use weak_self::WeakSelf;

type Vec2d = u16;

#[derive(Debug, Copy, Clone, EnumDiscriminants)]
enum Atom {
    // Pos(Vec2d),
    // Direction(Vec2d),
    Entity(u8),
    #[cfg(test)]
    TestUsize(usize),
}

/* Link: Basically a broadcast event bus.
 *
 *      source
 *         |
 *         v
 *  +----------------+
 *  |     Link       |
 *  +----------------+
 *  | |typ|          |
 *  | |latest_value| |
 *  +----------------+
 *      | | | |
 *      v v v v
 *  (... |callbacks|)
 */

struct Link {
    typ: AtomDiscriminants,
    latest_value: Option<Atom>,
    sinks: Vec<InputParameter>,
}

impl Link {
    fn new(typ: AtomDiscriminants) -> Self {
        Self {
            typ,
            latest_value: None,
            sinks: vec![],
        }
    }
    fn update(&mut self, next: Atom) {
        assert_eq!(self.typ, next.into());
        self.latest_value = Some(next);
        for sink in self.sinks.iter_mut() {
            InputParameter::mark_changed(sink)
        }
    }
    fn get_latest(&self) -> Option<Atom> {
        self.latest_value
    }
    fn add_sink(&mut self, sink: &InputParameter) {
        self.sinks.push(sink.clone());
    }
}

/* Defines behavior and the shape of node state.
 * Each Node in a power graph has a reference to
 */
trait NodeTemplate {
    fn in_types(&self) -> Vec<AtomDiscriminants>;
    fn out_types(&self) -> Vec<AtomDiscriminants>;
    fn create(&self) -> Arc<RefCell<dyn Node>>;
}

type CallbackRef = Arc<dyn Fn(Atom) -> ()>;
type InLinkList = Vec<Option<Arc<RefCell<Link>>>>;
type OutLinkList = Vec<Arc<RefCell<Link>>>;

trait Node {
    fn in_links(&self) -> &InLinkList;
    fn out_links(&self) -> &OutLinkList;
    fn template(&self) -> &Arc<dyn NodeTemplate>;
    fn get_callback_ref(&self, idx: usize) -> CallbackRef;
    #[cfg(test)]
    fn state(&self) -> Arc<dyn Any>;
}

type CallbackFn = Arc<dyn Fn(Atom, &OutLinkList) -> ()>;

trait NodeState: Default + Any {
    fn callback_fns(self: &Arc<Self>) -> Vec<CallbackFn>;
}

struct SimpleNode<StateT: NodeState> {
    in_links: InLinkList,
    template: Arc<dyn NodeTemplate>,
    state: Arc<StateT>,
    out_links: OutLinkList,
    callback_refs: Option<RefCell<Vec<CallbackRef>>>,
}

/* We want to be able to make new Node impls with minimal repeated code
 * Node impls need to provide
     * State (in struct, I guess?)
     * Behavior
        * Just a list of functions
        * Might modify state
     * Ability to hook up new callbacks
        * A trait-implemented function that converts behavior into list of closures.
 * 
 * We also want callbacks to be clear.
 *
 */

impl<T: NodeState> Node for SimpleNode<T> {
    fn in_links(&self) -> &InLinkList {
        &self.in_links
    }
    fn out_links(&self) -> &OutLinkList {
        &self.out_links
    }
    fn template(&self) -> &Arc<dyn NodeTemplate> {
        &self.template
    }
    fn get_callback_ref(&self, idx: usize) -> CallbackRef {
        assert!(self.callback_refs.is_some());
        self.callback_refs.as_ref().unwrap().borrow()[idx].clone()
    }
    #[cfg(test)]
    fn state(&self) -> Arc<dyn Any> {
        self.state.clone()
    }
}

impl<StateT: NodeState + 'static> SimpleNode<StateT> {
    pub fn from_template(template: Arc<dyn NodeTemplate>) -> Arc<RefCell<SimpleNode<StateT>>> {
        fn initialize_callback_refs<T: NodeState + 'static>(node_ref: &Arc<RefCell<SimpleNode<T>>>) {
            let mut node = node_ref.borrow_mut();
            if node.callback_refs.is_none() {
                node.callback_refs = Some(RefCell::new(T::callback_fns(&node.state).into_iter().map(
                    |f: CallbackFn| -> CallbackRef {
                        let inner_node_ref: Weak<RefCell<SimpleNode<T>>> = Arc::downgrade(&node_ref);
                        Arc::new(move |atom| {
                            inner_node_ref.upgrade().map(
                                |inner_node| f(atom, &inner_node.borrow().out_links)
                            );
                        })
                    }
                ).collect()));
            }
        }

        let in_links = template.in_types().iter().map(|_typ| None).collect();
        let out_links = template
            .out_types()
            .iter()
            .map(|typ| Arc::new(RefCell::new(Link::new(*typ))))
            .collect();
        let state = Arc::new(Default::default());
        let ret = Arc::new(RefCell::new(Self {
            in_links,
            out_links,
            template: template.clone(),
            state,
            callback_refs: None,
        }));
        initialize_callback_refs(&ret);
        return ret;
    }
}

#[derive(Clone)]
struct InputParameter {
    node: Weak<RefCell<dyn Node>>,
    idx: usize,
    typ: AtomDiscriminants,
}

impl InputParameter {
    pub fn mark_changed(&self) {
        // TODO: propagate somehow
    }
}

fn in_params(node: &Arc<RefCell<dyn Node>>) -> Vec<InputParameter> {
    node.borrow()
        .template()
        .in_types()
        .iter()
        .enumerate()
        .map(|(idx, typ)| InputParameter {
            node: Arc::downgrade(&node),
            idx,
            typ: *typ,
        })
        .collect()
}

#[derive(Clone)]
struct OutputParameter {
    node: Weak<RefCell<dyn Node>>,
    idx: usize,
    typ: AtomDiscriminants,
}

fn out_params(node: &Arc<RefCell<dyn Node>>) -> Vec<OutputParameter> {
    node.borrow()
        .template()
        .out_types()
        .iter()
        .enumerate()
        .map(|(idx, typ)| OutputParameter {
            node: Arc::downgrade(&node),
            idx,
            typ: *typ,
        })
        .collect()
}

fn attach(from_param: &OutputParameter, to_param: &InputParameter) {
    if let Some(src_ref) = from_param.node.upgrade() {
        let src = src_ref.borrow_mut();

        assert_eq!(from_param.typ, to_param.typ);

        src.out_links()[from_param.idx]
            // .as_ref()
            .borrow_mut()
            .add_sink(&to_param);
        // TODO: Attach in-link (not used, but good for bidirectionally showing structure of graph)
    }
}

/*
fn generate(templates: Vec<Box<dyn NodeTemplate>>) {
    Vec<Box<
}
*/

#[cfg(test)]
#[derive(Default)]
struct EmitUsizeState {}

#[cfg(test)]
impl NodeState for EmitUsizeState {
    fn callback_fns(self: &Arc<Self>) -> Vec<CallbackFn> {
        vec![]
    }
}

#[cfg(test)]
struct EmitUsizeTemplate {
    weak_self: WeakSelf<Self>,
}

#[cfg(test)]
impl EmitUsizeTemplate {
    fn new() -> Arc<Self> {
        let ret = Arc::new(
            Self {
                weak_self: WeakSelf::new(),
            }
        );
        ret.weak_self.init(&ret);
        ret.into()
    }
}

#[cfg(test)]
impl NodeTemplate for EmitUsizeTemplate {
    fn in_types(&self) -> Vec<AtomDiscriminants> {
        vec![]
    }
    fn out_types(&self) -> Vec<AtomDiscriminants> {
        vec![AtomDiscriminants::TestUsize]
    }
    fn create(&self) -> Arc<RefCell<dyn Node>> {
        SimpleNode::<EmitUsizeState>::from_template(self.weak_self.get().upgrade().unwrap())
    }
}

#[cfg(test)]
#[derive(Default)]
struct TakeUsizeState {
    received: usize,
}

/* Does NodeState need access to a weak-ptr to itself?
 * I guess it does.
 * How to supply?
 *
 */

#[cfg(test)]
impl TakeUsizeState {
    fn take(&mut self, recv: usize) {
        self.received = recv;
    }
}

#[cfg(test)]
impl NodeState for RefCell<TakeUsizeState> {
    fn callback_fns(self: &Arc<Self>) -> Vec<CallbackFn> {
        let weak_self = Arc::downgrade(self);
        vec![
            Arc::new(move |atom: Atom, links: &OutLinkList| -> () {
                if let Atom::TestUsize(v) = atom {
                    weak_self.upgrade().map(|self_| {
                        self_.borrow_mut().received = v;
                    });
                };
            })
        ]
    }
}

#[cfg(test)]
struct TakeUsizeTemplate {
    weak_self: WeakSelf<Self>,
}

#[cfg(test)]
impl TakeUsizeTemplate {
    fn new() -> Arc<Self> {
        let ret = Arc::new(
            Self {
                weak_self: WeakSelf::new(),
            }
        );
        ret.weak_self.init(&ret);
        ret
    }
}


#[cfg(test)]
impl NodeTemplate for TakeUsizeTemplate {
    fn in_types(&self) -> Vec<AtomDiscriminants> {
        vec![AtomDiscriminants::TestUsize]
    }
    fn out_types(&self) -> Vec<AtomDiscriminants> {
        vec![]
    }
    fn create(&self) -> Arc<RefCell<dyn Node>> {
        SimpleNode::<RefCell<TakeUsizeState>>::from_template(self.weak_self.get().upgrade().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn it_works() {
        let a_sig = EmitUsizeTemplate::new();
        let b_sig = TakeUsizeTemplate::new();

        let a = a_sig.create();
        let b = b_sig.create();

        attach(&out_params(&a)[0], &in_params(&b)[0]);

        a.borrow().out_links()[0].borrow_mut().update(Atom::TestUsize(5));
        assert_eq!(5, b.borrow().state().downcast_ref::<RefCell<TakeUsizeState>>().unwrap().borrow().received);
    }
}
