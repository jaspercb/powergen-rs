// Strum contains all the trait definitions
extern crate strum;
#[macro_use]
extern crate strum_macros;

use std::any::Any;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::fmt::Debug;
use std::sync::{Arc, Weak};

use weak_self::WeakSelf;

#[derive(Debug, Copy, Clone, EnumDiscriminants)]
#[strum_discriminants(derive(Hash))]
enum Atom {
    // Pos(Vec2d),
    // Direction(Vec2d),
    Entity(u8),
    #[cfg(test)]
    TestUsize(usize),
}

/* Link: Basically a broadcast event bus inserted between nodes.
 *
 *       source
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
            sink.mark_changed(next);
        }
    }
    fn get_latest(&self) -> Option<Atom> {
        self.latest_value
    }
    fn add_sink(&mut self, sink: &InputParameter) {
        self.sinks.push(sink.clone());
    }
}

trait NodeTemplate {
    fn in_types(&self) -> Vec<AtomDiscriminants>;
    fn out_types(&self) -> Vec<AtomDiscriminants>;
    fn create(&self) -> Arc<RefCell<dyn Node>>;
}

impl Debug for NodeTemplate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Node {{ in_types: {:?}, out_types: {:?} }}",
            self.in_types(),
            self.out_types()
        )
    }
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

trait NodeState: Default + Any + Debug {
    fn callback_fns(self: Arc<Self>) -> Vec<CallbackFn>;
}

struct SimpleNode<StateT: NodeState> {
    in_links: InLinkList,
    template: Arc<dyn NodeTemplate>,
    state: Arc<StateT>,
    out_links: OutLinkList,
    callback_refs: Option<RefCell<Vec<CallbackRef>>>,
}

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
        fn initialize_callback_refs<T: NodeState + 'static>(
            node_ref: &Arc<RefCell<SimpleNode<T>>>,
        ) {
            let mut node = node_ref.borrow_mut();
            if node.callback_refs.is_none() {
                node.callback_refs = Some(RefCell::new(
                    T::callback_fns(node.state.clone())
                        .into_iter()
                        .map(|f: CallbackFn| -> CallbackRef {
                            let inner_node_ref: Weak<RefCell<SimpleNode<T>>> =
                                Arc::downgrade(&node_ref);
                            Arc::new(move |atom| {
                                inner_node_ref
                                    .upgrade()
                                    .map(|inner_node| f(atom, &inner_node.borrow().out_links));
                            })
                        })
                        .collect(),
                ));
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
    pub fn mark_changed(&self, value: Atom) {
        self.node
            .upgrade()
            .unwrap()
            .borrow()
            .get_callback_ref(self.idx)(value);
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
            .borrow_mut()
            .add_sink(&to_param);
        // TODO: Attach in-link (not used, but good for bidirectionally showing structure of graph)
    }
}

type TypeMultiset = HashMap<AtomDiscriminants, u8>;

fn contains(haystack: &TypeMultiset, needle: &TypeMultiset) -> bool {
    for (typ, num) in needle {
        if num > haystack.get(typ).unwrap_or(&0) {
            return false;
        }
    }
    true
}

fn type_set(types: Vec<AtomDiscriminants>) -> TypeMultiset {
    let mut counts = TypeMultiset::new();
    for typ in types {
        *counts.entry(typ.into()).or_insert(0) += 1;
    }
    counts
}

fn generate_graphs(templates: &Vec<Arc<dyn NodeTemplate>>) -> Vec<Vec<&Arc<dyn NodeTemplate>>> {
    /* Returns the input nodes of a generated power graph. Generation occurs in two stages:
     *
     * 1. Using type annotations, create a potential topsort of the graph's templates
     *  e.g. using types
     *      Source: () -> A
     *      Id: A -> A
     *      Sink: A -> ()
     *
     *  this phase could return [Source, Sink], [Source, Id, Sink], etc.
     *
     * 2. Turn that topsorted template into an instantiated power graph, linking up nodes as
     *    needed.
     */

    let templates_by_type: Vec<(TypeMultiset, &Arc<dyn NodeTemplate>, TypeMultiset)> = templates
        .iter()
        .map(|template| {
            (
                type_set(template.in_types()),
                template,
                type_set(template.out_types()),
            )
        })
        .collect();
    println!("templates_by_type: {:?}", templates_by_type);

    let mut search_q: VecDeque<(TypeMultiset, Vec<&Arc<dyn NodeTemplate>>)> = VecDeque::new();
    search_q.push_back((TypeMultiset::new(), Vec::new()));

    let mut results = Vec::new();

    for i in 1..5 {
        if let Some((available_types, prev_templates)) = search_q.pop_front() {
            for (in_type_set, next_template, out_type_set) in templates_by_type
                .iter()
                .filter(|(type_set, _, _)| contains(&available_types, type_set))
            {
                let mut next_types = available_types.clone();
                for (typ, count) in in_type_set {
                    next_types.entry(*typ).and_modify(|e| *e -= count);
                }
                for (typ, count) in out_type_set {
                    next_types
                        .entry(*typ)
                        .and_modify(|e| *e += count)
                        .or_insert(*count);
                }
                let mut next_templates = prev_templates.clone();
                next_templates.push(next_template);

                if next_types.iter().all(|(_k, count)| count == &0) {
                    search_q.push_back((next_types, next_templates));
                } else {
                    results.push(next_templates);
                }
            }
        }
    }
    results
}

#[cfg(test)]
#[derive(Default, Debug)]
struct EmitUsizeState {}

#[cfg(test)]
impl NodeState for EmitUsizeState {
    fn callback_fns(self: Arc<Self>) -> Vec<CallbackFn> {
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
        let ret = Arc::new(Self {
            weak_self: WeakSelf::new(),
        });
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
#[derive(Default, Debug)]
struct TakeUsizeState {
    received: usize,
}

#[cfg(test)]
impl NodeState for RefCell<TakeUsizeState> {
    fn callback_fns(self: Arc<Self>) -> Vec<CallbackFn> {
        let weak_self = Arc::downgrade(&self);
        vec![Arc::new(move |atom: Atom, links: &OutLinkList| -> () {
            if let Atom::TestUsize(v) = atom {
                weak_self.upgrade().map(|self_| {
                    self_.borrow_mut().received = v;
                });
            } else {
                panic!("wtf");
            }
        })]
    }
}

#[cfg(test)]
struct TakeUsizeTemplate {
    weak_self: WeakSelf<Self>,
}

#[cfg(test)]
impl TakeUsizeTemplate {
    fn new() -> Arc<Self> {
        let ret = Arc::new(Self {
            weak_self: WeakSelf::new(),
        });
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
        SimpleNode::<RefCell<TakeUsizeState>>::from_template(
            self.weak_self.get().upgrade().unwrap(),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::*;
    #[test]
    fn can_link_nodes() {
        let a_sig = EmitUsizeTemplate::new();
        let b_sig = TakeUsizeTemplate::new();

        let a = a_sig.create();
        let b = b_sig.create();

        attach(&out_params(&a)[0], &in_params(&b)[0]);

        a.borrow().out_links()[0]
            .borrow_mut()
            .update(Atom::TestUsize(5));
        assert_eq!(
            5,
            b.borrow()
                .state()
                .downcast_ref::<RefCell<TakeUsizeState>>()
                .unwrap()
                .borrow()
                .received
        );
    }

    #[test]
    fn can_generate_graphs() {
        let templates: Vec<Arc<dyn NodeTemplate>> =
            vec![EmitUsizeTemplate::new(), TakeUsizeTemplate::new()];

        let results = generate_graphs(&templates);
        assert_eq!(1, results.len());
    }

    #[test]
    fn type_multiset_works() {
        let empty = TypeMultiset::new();
        let mut one = TypeMultiset::new();
        one.insert(AtomDiscriminants::Entity, 1);
        let mut two = TypeMultiset::new();
        two.insert(AtomDiscriminants::Entity, 2);

        assert_eq!(true, contains(&empty, &empty));

        assert_eq!(true, contains(&one, &empty));
        assert_eq!(false, contains(&empty, &one));

        assert_eq!(true, contains(&two, &one));
        assert_eq!(false, contains(&one, &two));
    }
}
