// Strum contains all the trait definitions
extern crate strum;
#[macro_use]
extern crate strum_macros;

use std::cell::RefCell;
use std::rc::Rc;

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

type WrappedCallback = Rc<RefCell<Box<dyn CallbackParameter>>>;

struct Link {
    typ: AtomDiscriminants,
    latest_value: Option<Atom>,
    callbacks: Vec<WrappedCallback>,
}

impl Link {
    fn new(typ: AtomDiscriminants) -> Self {
        Self {
            typ,
            latest_value: None,
            callbacks: vec![],
        }
    }
    fn update(&mut self, next: Atom) {
        assert_eq!(self.typ, next.into());
        self.latest_value = Some(next);
        for cb in self.callbacks.iter() {
            cb.borrow().call(next);
        }
    }
    fn get_latest(&self) -> Option<Atom> {
        self.latest_value
    }
    fn add_callback(&mut self, callback: &WrappedCallback) {
        self.callbacks.push(callback.clone());
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
    in_links: Vec<Option<Rc<RefCell<Link>>>>,
    out_links: Vec<Rc<RefCell<Link>>>,
    template: Rc<dyn NodeTemplate<StateT>>,
    state: Rc<RefCell<StateT>>,
    callback_params: Vec<WrappedCallback>,
}

impl<StateT: NodeState + 'static> Node<StateT> {
    pub fn from_template(template: Rc<dyn NodeTemplate<StateT>>) -> Rc<RefCell<Node<StateT>>> {
        let in_links = template.in_types().iter().map(|_typ| None).collect();
        let out_links = template
            .out_types()
            .iter()
            .map(|typ| Rc::new(RefCell::new(Link::new(*typ))))
            .collect();
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
            .map(|param: InputParameter<StateT>| Rc::new(RefCell::new(Box::new(param) as Box<dyn CallbackParameter>)))
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
        self.node.borrow().template.callbacks()[self.idx](
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

fn out_params<T: NodeState>(node: &Rc<RefCell<Node<T>>>) -> Vec<OutputParameter<T>> {
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

fn attach<A: NodeState, B: NodeState>(from_param: &OutputParameter<A>, to_param: &InputParameter<B>) {
    let src = from_param.node.borrow_mut();
    let sink = to_param.node.borrow_mut();

    // Assert the output of src and the input of sink are the same type.
    assert_eq!(from_param.typ, to_param.typ);

    let callback = sink.callback_params[to_param.idx].clone();

    src.out_links[from_param.idx]
        // .as_ref()
        .borrow_mut()
        .add_callback(&callback);
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
            |state: Rc<RefCell<TestTakeUsizeSignature>>, atom: Atom| {
                println!("hi");
                match atom {
                    Atom::TestUsize(v) => state.borrow_mut().received = v,
                    _ => (),
                }
            },
        )]
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn it_works() {
        let a_sig = Rc::new(TestEmitUsizeSignatureTemplate {});
        let b_sig = Rc::new(TestTakeUsizeSignatureTemplate {});

        let a = Node::from_template(a_sig);
        let b = Node::from_template(b_sig);

        attach(&out_params(&a)[0], &in_params(&b)[0]);

        a.borrow().out_links[0].borrow_mut().update(Atom::TestUsize(5));
        assert_eq!(5, b.borrow().state.borrow().received);
    }
}
