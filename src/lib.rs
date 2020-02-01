// Strum contains all the trait definitions
extern crate strum;
#[macro_use]
extern crate strum_macros;

use std::cell::RefCell;
use std::rc::{Rc, Weak};

type Vec2d = u16;

trait NodeState: Default {}
impl<T: Default> NodeState for T {}

#[derive(Debug, Copy, Clone, EnumDiscriminants)]
enum Atom {
    Pos(Vec2d),
    Direction(Vec2d),
    Entity(u8),
    #[cfg(test)]
    TestUsize(usize),
}

struct Link {
    latest_value: Option<Atom>,
    callbacks: Vec<Rc<dyn CallbackParameter>>,
}

impl Link {
    fn new() -> Self {
        Self {
            latest_value: None,
            callbacks: vec![],
        }
    }
    fn update(&mut self, next: Atom) {
        self.latest_value = Some(next);
    }
    fn get_latest(&self) -> Option<Atom> {
        self.latest_value
    }
    fn add_callback(&mut self, callback: Rc<dyn CallbackParameter>) {
        self.callbacks.push(callback);
    }
}

/* Defines behavior and the shape of node state.
 * Each Node in a power graph has a reference to
 */
trait NodeTemplate<StateT: NodeState> {
    fn in_types(&self) -> Vec<AtomDiscriminants>;
    fn out_types(&self) -> Vec<AtomDiscriminants>;
    fn callbacks(&self) -> Vec<Rc<dyn Fn(Rc<RefCell<StateT>>, Atom)>>;
}

struct Node<StateT: NodeState> {
    in_links: Vec<Rc<RefCell<Link>>>,
    out_links: Vec<Option<Rc<RefCell<Link>>>>,
    template: Rc<dyn NodeTemplate<StateT>>,
    state: Rc<RefCell<StateT>>,
    callback_params: Vec<Rc<dyn CallbackParameter>>,
}

impl<StateT: NodeState + 'static> Node<StateT> {
    pub fn from_template(template: Rc<dyn NodeTemplate<StateT>>) -> Rc<RefCell<Node<StateT>>> {
        let in_links = template
            .in_types()
            .iter()
            .map(|_typ| Rc::new(RefCell::new(Link::new())))
            .collect();
        let out_links = template.out_types().iter().map(|_typ| None).collect();
        let state = Rc::new(RefCell::new(Default::default()));
        let ret = Rc::new(RefCell::new(Self {
            in_links,
            out_links,
            template: template.clone(),
            state,
            callback_params: Vec::new(),
        }));

        ret.borrow_mut().callback_params = in_params(&ret)
            .into_iter()
            .map(|param: InputParameter<StateT>| Rc::new(param) as Rc<dyn CallbackParameter>)
            .collect();
        return ret;
    }
}

struct InputParameter<StateT: NodeState> {
    node: Rc<RefCell<Node<StateT>>>,
    idx: usize,
    typ: AtomDiscriminants,
}

fn in_params<T: NodeState>(node: &Rc<RefCell<Node<T>>>) -> Vec<InputParameter<T>> {
    node.borrow()
        .template
        .in_types()
        .iter()
        .enumerate()
        .map(|(idx, typ)| InputParameter {
            node: node.clone(),
            idx,
            typ: *typ,
        })
        .collect()
}

// Basically still an input parameter
trait CallbackParameter {
    fn call(&self, atom: Atom) -> ();
}

impl<StateT: NodeState> CallbackParameter for InputParameter<StateT> {
    fn call(&self, atom: Atom) -> () {
        self.node.borrow_mut().template.callbacks()[self.idx](
            self.node.borrow().state.clone(),
            atom,
        );
    }
}

struct OutputParameter<StateT: NodeState> {
    node: Rc<RefCell<Node<StateT>>>,
    idx: usize,
    typ: AtomDiscriminants,
}

fn out_params<T: NodeState>(node: Rc<RefCell<Node<T>>>) -> Vec<OutputParameter<T>> {
    node.borrow()
        .template
        .out_types()
        .iter()
        .enumerate()
        .map(|(idx, typ)| OutputParameter {
            node: node.clone(),
            idx,
            typ: *typ,
        })
        .collect()
}

// trait NodeState

/* I feel like I'm moving in circles here
 * Node:
 *  Owned NodeStruct field
 *  Rc to NodeTemplate
 *  Logic for graph modification
 *  Struct.
 *
 * NodeState:
 *  Holds data.
 *  Struct.
 *
 * NodeTemplate:
 *  Holds behavior.
 *  Trait.
 *
 * Currently everything is templated on NodeState
 *   This doesn't necessarily make a ton of sense
 */

// fn attach<A: NodeState, B: NodeState>(ref_src: Rc<RefCell<Node<A>>>, out_idx: usize, ref_sink: Rc<RefCell<Node<B>>>, in_idx: usize) {
fn attach<A: NodeState, B: NodeState>(from_param: OutputParameter<A>, to_param: InputParameter<B>) {
    let mut src = from_param.node.borrow_mut();
    let mut sink = to_param.node.borrow_mut();

    // Assert the output of src and the input of sink are the same type.
    assert!(from_param.typ == to_param.typ);

    // If it doesn't already exist, create a link associated with source.
    if src.out_links[from_param.idx].is_none() {
        src.out_links[from_param.idx] = Some(Rc::new(RefCell::new(Link::new())));
    }

    let callback = sink.callback_params[to_param.idx].clone();

    src.out_links[from_param.idx]
        .as_ref()
        .unwrap()
        .borrow_mut()
        .add_callback(callback);

    // TODO: Add the sink as a listener.
}

/* What do I need to specify a node?
 *   In-types
 *   Out-types
 *   What it does with its in-types (one callback function per)
 *   [maybe future] Time constraints
 *
 * What can be shared?
 *   Instantiation: create empty in-links and out-links
 *
 */

#[cfg(test)]
#[derive(Default)]
struct TestEmitUsizeSignature {}

#[cfg(test)]
struct TestEmitUsizeSignatureTemplate {}

#[cfg(test)]
impl NodeTemplate<TestEmitUsizeSignature> for TestEmitUsizeSignatureTemplate {
    fn in_types(&self) -> Vec<AtomDiscriminants> {
        vec![]
    }
    fn out_types(&self) -> Vec<AtomDiscriminants> {
        vec![AtomDiscriminants::TestUsize]
    }
    fn callbacks(&self) -> Vec<Rc<dyn Fn(Rc<RefCell<TestEmitUsizeSignature>>, Atom)>> {
        vec![]
    }
}

#[cfg(test)]
#[derive(Default)]
struct TestTakeUsizeSignature {
    received: usize,
}

#[cfg(test)]
struct TestTakeUsizeSignatureTemplate {}

#[cfg(test)]
impl NodeTemplate<TestTakeUsizeSignature> for TestTakeUsizeSignatureTemplate {
    fn in_types(&self) -> Vec<AtomDiscriminants> {
        vec![AtomDiscriminants::TestUsize]
    }
    fn out_types(&self) -> Vec<AtomDiscriminants> {
        vec![]
    }
    fn callbacks(&self) -> Vec<Rc<dyn Fn(Rc<RefCell<TestTakeUsizeSignature>>, Atom)>> {
        vec![Rc::new(
            |state: Rc<RefCell<TestTakeUsizeSignature>>, atom: Atom| match atom {
                Atom::TestUsize(v) => state.borrow_mut().received = v,
                _ => (),
            },
        )]
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        // let sig = TestTakeUsizeSignature
    }
}
