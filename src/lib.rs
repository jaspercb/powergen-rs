// Strum contains all the trait definitions
extern crate strum;
#[macro_use]
extern crate strum_macros;

use std::cell::RefCell;
use std::rc::{Rc, Weak};


type Vec2d = u16;

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
    callbacks: Vec<Rc<dyn NodeCallback>>,
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
    fn add_callback(&mut self, callback: Rc<dyn NodeCallback>) {
        self.callbacks.push(callback);
    }
}

trait NodeTemplate<DataT> {
    fn in_types(&self) -> Vec<AtomDiscriminants>;
    fn out_types(&self) -> Vec<AtomDiscriminants>;
    fn callbacks(&self) -> Vec<Rc<dyn Fn(Rc<RefCell<DataT>>, Atom)>>;
}

struct Node<DataT> {
    in_links: Vec<Option<Rc<RefCell<Link>>>>,
    out_links: Vec<Option<Rc<RefCell<Link>>>>,
    template: Rc<dyn NodeTemplate<DataT>>,
    data: Rc<RefCell<DataT>>,
}


// TODO: pair Node<T> and NodeTemplate<T> together behind a trait?
// Trait supports

impl<T> Node<T> {

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

trait NodeCallback {
    fn call(&self, atom: Atom) -> ();
}

#[derive(Clone)]
struct NodeInputCallback<T> {
    node: Rc<RefCell<Node<T>>>,
    idx: usize,
}

impl<DataT> NodeCallback for NodeInputCallback<DataT> {
    fn call(&self, atom: Atom) -> () {
        self.node.borrow_mut().template.callbacks()[self.idx](self.node.borrow().data.clone(), atom);
    }
}

fn attach<A, B: 'static>(ref_src: Rc<RefCell<Node<A>>>, out_idx: usize, ref_sink: Rc<RefCell<Node<B>>>, in_idx: usize) {
    let mut src = ref_src.borrow_mut();
    let mut sink = ref_sink.borrow_mut();

    // Assert the output of src and the input of sink are the same type.
    let out_type = src.template.out_types()[out_idx];
    let in_type = sink.template.in_types()[in_idx];
    assert!(out_type == in_type);

    // If it doesn't already exist, create a link associated with source.
    if src.out_links[out_idx].is_none() {
        src.out_links[out_idx] = Some(Rc::new(RefCell::new(Link::new())));
    }

    src.out_links[out_idx].as_ref().unwrap().borrow_mut().add_callback(
        Rc::new(NodeInputCallback {
            node: ref_sink.clone(), idx: in_idx,
        })
    );

    // Add the sink as a listener.
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
        vec![
            Rc::new(|state: Rc<RefCell<TestTakeUsizeSignature>>, atom: Atom| {
                match atom {
                    Atom::TestUsize(v) => state.borrow_mut().received = v,
                    _ => ()
                }
            })
        ]
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn it_works() {
        // let sig = TestTakeUsizeSignature

    }
}
