#[cfg(test)]
mod test;

use arrayvec::ArrayVec;
use blink_alloc::Blink;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
use std::ops::DerefMut;
use std::slice::Iter;

const MAX_ITEMS: usize = 32;
const MIN_ITEMS: usize = 2;

#[derive(Copy, Clone)]
enum Axis {
    X,
    Y,
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    fn on(self, axis: Axis) -> f32 {
        match axis {
            Axis::X => self.x,
            Axis::Y => self.y,
        }
    }
}

#[derive(Clone, Copy, PartialEq, Default, Debug)]
pub struct Rect {
    pub min: Point,
    pub max: Point,
}

impl Rect {
    pub fn new(min: Point, max: Point) -> Self {
        Self { min, max }
    }

    pub fn point(x: f32, y: f32) -> Self {
        Self {
            min: Point { x, y },
            max: Point { x, y },
        }
    }

    fn expand(&mut self, rect: &Self) {
        if rect.min.x < self.min.x {
            self.min.x = rect.min.x;
        }
        if rect.max.x > self.max.x {
            self.max.x = rect.max.x;
        }
        if rect.min.y < self.min.y {
            self.min.y = rect.min.y;
        }
        if rect.max.y > self.max.y {
            self.max.y = rect.max.y;
        }
    }

    fn largest_axis(&self) -> Axis {
        let x = self.max.x - self.min.x;
        let y = self.max.y - self.min.y;
        if y > x {
            Axis::Y
        } else {
            Axis::X
        }
    }

    fn intersects(&self, rect: &Self) -> bool {
        if rect.min.x > self.max.x || rect.max.x < self.min.x {
            return false;
        }
        if rect.min.y > self.max.y || rect.max.y < self.min.y {
            return false;
        }
        true
    }

    fn on_edge(&self, rect: &Self) -> bool {
        if !(rect.min.x > self.min.x) || !(rect.max.x < self.max.x) {
            return true;
        }
        if !(rect.min.y > self.min.y) || !(rect.max.y < self.max.y) {
            return true;
        }
        false
    }

    fn area(&self) -> f32 {
        let x = self.max.x - self.min.x;
        let y = self.max.y - self.min.y;
        x * y
    }

    fn unioned_area(&self, rect: &Rect) -> f32 {
        let x = max(self.max.x, rect.max.x) - min(self.min.x, rect.min.x);
        let y = max(self.max.y, rect.max.y) - min(self.min.y, rect.min.y);
        x * y
    }

    pub fn box_dist(&self, rect: &Rect) -> f32 {
        let x = max(self.min.x, rect.min.x) - min(self.max.x, rect.max.x);
        let y = max(self.min.y, rect.min.y) - min(self.max.y, rect.max.y);
        x * x + y * y
    }
}

trait Tx<T> {
    type P: DerefMut<Target = T>;

    fn a(self, value: T) -> Self::P;
}

impl<'a, T: 'a> Tx<T> for &'a Blink {
    type P = &'a mut T;

    fn a(self, value: T) -> Self::P {
        self.put_no_drop(value)
    }
}

trait Alloc<T> {
    type Ptr<'a>: DerefMut<Target = T>
    where
        T: 'a,
        Self: 'a;

    fn alloc(&self, value: T) -> Self::Ptr<'_>;
}

struct BoxAlloc;

impl<T: 'static> Alloc<T> for BoxAlloc {
    type Ptr<'a> = Box<T>;

    fn alloc(&self, value: T) -> Self::Ptr<'static> {
        Box::new(value)
    }
}

impl<T> Alloc<T> for Blink {
    type Ptr<'a> = &'a mut T where T: 'a;

    fn alloc(&self, value: T) -> Self::Ptr<'_> {
        self.put_no_drop(value)
    }
}

type NodeVec<'n, T> = ArrayVec<Node<'n, T>, MAX_ITEMS>;

struct Parent<'n, T: 'n> {
    nodes: &'n mut NodeVec<'n, T>,
    rect: Rect,
}

impl<'n, T: 'n> Parent<'n, T> {
    fn new(rect: Rect, blink: &'n Blink) -> Self {
        Self {
            nodes: blink.put_no_drop(ArrayVec::new()),
            rect,
        }
    }

    fn len(&self) -> usize {
        self.nodes.len()
    }

    fn is_full(&self) -> bool {
        self.nodes.is_full()
    }

    fn choose_least_enlargement(&mut self, rect: &Rect) -> &mut Node<'n, T> {
        let mut n = None;
        let mut min_delta = 0.0;
        let mut min_area = 0.0;
        for node in self.nodes.iter_mut() {
            let uarea = node.rect().unioned_area(rect);
            let area = node.rect().area();
            let delta = uarea - area;
            if n.is_none() || delta < min_delta || (delta == min_delta && area < min_area) {
                n = Some(node);
                min_delta = delta;
                min_area = area;
            }
        }
        n.expect("empty parent")
    }

    fn insert(&mut self, rect: Rect, item: T, height: usize, blink: &'n Blink) {
        if height > 0 {
            // branch node
            let Node::Parent(child) = self.choose_least_enlargement(&rect) else {
                return;
            };
            child.insert(rect, item, height - 1, blink);
            if child.is_full() {
                let right = child.split_largest_axis_edge_snap(blink);
                self.nodes.push(right);
            }
        } else {
            // leaf node
            self.nodes.push(Node::Item(Item { rect, item }));
        }
        self.rect.expand(&rect);
    }

    fn recalc(&mut self) {
        if self.nodes.len() == 0 {
            return;
        }
        let mut rect = self.nodes[0].rect().clone();
        for i in 1..self.nodes.len() {
            rect.expand(&self.nodes[i].rect());
        }
        self.rect = rect;
    }

    fn split_largest_axis_edge_snap(&mut self, blink: &'n Blink) -> Node<'n, T> {
        let rect = self.rect;
        let axis = rect.largest_axis();
        let mut right = Parent::new(rect, blink);
        let lchilds = &mut self.nodes;
        let rchilds = &mut right.nodes;
        let mut i = 0;
        while i < lchilds.len() {
            let min = lchilds[i].rect().min.on(axis) - rect.min.on(axis);
            let max = rect.max.on(axis) - lchilds[i].rect().max.on(axis);
            if min < max {
                // stay left
                i += 1;
            } else {
                // move right
                rchilds.push(lchilds.swap_remove(i));
            }
        }
        // Make sure that both left and right nodes have at least
        // MIN_ITEMS by moving items into under-flowed nodes.
        if lchilds.len() < MIN_ITEMS {
            // reverse sort by min axis
            rchilds.sort_unstable_by_key(|n| Ordered(n.rect().min.on(axis)));
            while lchilds.len() < MIN_ITEMS {
                lchilds.push(rchilds.pop().unwrap());
            }
        } else if rchilds.len() < MIN_ITEMS {
            // reverse sort by max axis
            lchilds.sort_unstable_by_key(|n| Ordered(n.rect().max.on(axis)));
            while rchilds.len() < MIN_ITEMS {
                rchilds.push(lchilds.pop().unwrap());
            }
        }
        // recalculate and sort the nodes
        self.recalc();
        right.recalc();
        self.sort_by_x();
        right.sort_by_x();
        Node::Parent(right)
    }

    fn push(&mut self, child: Node<'n, T>) {
        self.nodes.push(child);
    }

    fn sort_by_x(&mut self) {
        self.nodes.sort_unstable_by_key(|n| Ordered(n.rect().min.x));
    }

    fn flatten_into(&mut self, reinsert: &mut Vec<Item<T>>) {
        while let Some(node) = self.nodes.pop() {
            match node {
                Node::Item(item) => reinsert.push(item),
                Node::Parent(mut nodes) => nodes.flatten_into(reinsert),
            }
        }
    }

    pub fn remove(
        &mut self,
        rect: &Rect,
        data: &T,
        reinsert: &mut Vec<Item<T>>,
        height: usize,
    ) -> (Option<Item<T>>, bool)
    where
        T: PartialEq,
    {
        let nodes = &mut self.nodes;
        if height == 0 {
            // remove from leaf
            for i in 0..nodes.len() {
                if nodes[i].item() != data {
                    continue;
                }
                let Node::Item(item) = nodes.swap_remove(i) else {
                    continue;
                };
                let recalced = self.rect.on_edge(&item.rect);
                if recalced {
                    self.recalc();
                }
                return (Some(item), recalced);
            }
        } else {
            for i in 0..nodes.len() {
                let node = nodes[i].nodes_mut();
                if !node.rect.intersects(rect) {
                    continue;
                }
                let (removed, mut recalced) = node.remove(rect, data, reinsert, height - 1);
                if removed.is_none() {
                    continue;
                }
                let underflow = node.len() < MIN_ITEMS;
                if underflow {
                    let nrect = node.rect;
                    nodes.swap_remove(i).nodes_mut().flatten_into(reinsert);
                    if !recalced {
                        recalced = self.rect.on_edge(&nrect);
                    }
                }
                if recalced {
                    self.recalc();
                }
                return (removed, recalced);
            }
        }
        (None, false)
    }

    pub fn search_flat<'this>(&'this self, rect: &Rect, items: &mut Vec<(Rect, &'this T)>) {
        for node in self.nodes.iter() {
            if node.rect().intersects(&rect) {
                match node {
                    Node::Item(item) => items.push((item.rect, &item.item)),
                    Node::Parent(nodes) => nodes.search_flat(&rect, items),
                }
            }
        }
    }
}

pub struct Item<T> {
    rect: Rect,
    item: T,
}

enum Node<'n, T: 'n> {
    Item(Item<T>),
    Parent(Parent<'n, T>),
}

impl<'n, T: 'n> Node<'n, T> {
    fn rect(&self) -> &Rect {
        match self {
            Node::Item(n) => &n.rect,
            Node::Parent(n) => &n.rect,
        }
    }

    fn item(&self) -> &T {
        match self {
            Node::Item(n) => &n.item,
            Node::Parent(_) => panic!("not a leaf node"),
        }
    }

    fn into_item(self) -> T {
        match self {
            Node::Item(n) => n.item,
            Node::Parent(_) => panic!("not a leaf node"),
        }
    }

    fn nodes(&self) -> &Parent<'n, T> {
        match self {
            Node::Item(_) => panic!("not a parent node"),
            Node::Parent(n) => n,
        }
    }

    fn nodes_mut(&mut self) -> &mut Parent<'n, T> {
        match self {
            Node::Item(_) => panic!("not a parent node"),
            Node::Parent(n) => n,
        }
    }
}

pub struct RTree<'n, T: 'n> {
    blink: &'n Blink,
    root: Option<Node<'n, T>>,
    length: usize,
    height: usize,
}

impl<'n, T: 'n> RTree<'n, T> {
    pub fn new(blink: &'n Blink) -> Self {
        RTree {
            blink,
            root: None,
            length: 0,
            height: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn rect(&self) -> Option<Rect> {
        self.root.as_ref().map(|root| root.rect().clone())
    }

    pub fn insert(&mut self, rect: Rect, data: T) {
        let root = self
            .root
            .get_or_insert_with(|| Node::Parent(Parent::new(rect, &self.blink)))
            .nodes_mut();
        root.insert(rect, data, self.height, &self.blink);
        if root.is_full() {
            let mut new_root = Parent::new(root.rect, &self.blink);
            let right = root.split_largest_axis_edge_snap(&self.blink);
            let left = self.root.take().unwrap();
            new_root.push(left);
            new_root.push(right);
            self.root = Some(Node::Parent(new_root));
            self.height += 1;
        }
        self.length += 1;
    }

    pub fn remove(&mut self, rect: Rect, data: &T) -> Option<Item<T>>
    where
        T: PartialEq,
    {
        if let Some(root) = &mut self.root {
            let root = root.nodes_mut();
            let mut reinsert = Vec::new();
            let (removed, recalced) = root.remove(&rect, data, &mut reinsert, self.height);
            if removed.is_none() {
                return None;
            }
            self.length -= reinsert.len() + 1;
            if self.length == 0 {
                self.root = None;
            } else if self.height > 0 && root.len() == 1 {
                let mut n = root.nodes.pop().unwrap();
                n.nodes_mut().recalc();
                self.height -= 1;
                self.root = Some(n);
            } else if recalced {
                if let Some(root) = &mut self.root {
                    root.nodes_mut().recalc();
                }
            }
            while let Some(item) = reinsert.pop() {
                self.insert(item.rect, item.item);
            }
            removed
        } else {
            None
        }
    }

    pub fn search_flat<'this>(&'this self, rect: Rect, items: &mut Vec<(Rect, &'this T)>) {
        if let Some(root) = &self.root {
            root.nodes().search_flat(&rect, items);
        }
    }

    pub fn iter<'this>(&'this self) -> ScanIterator<'n, 'this, T> {
        ScanIterator::new(self.root.as_ref().map(Node::nodes), self.height)
    }

    pub fn search<'this>(&'this self, rect: Rect) -> SearchIterator<'n, 'this, T> {
        SearchIterator::new(self.root.as_ref().map(Node::nodes), self.height, rect)
    }

    pub fn nearby<'this, F>(&'this self, dist: F) -> NearbyIterator<'n, 'this, T, F>
    where
        F: FnMut(&Rect, Option<&'this T>) -> f32,
    {
        NearbyIterator::new(&self.root, dist)
    }
}

// iterators, ScanIterator, SearchIterator, NearbyIterator

pub struct IterItem<'n, T> {
    pub rect: Rect,
    pub data: &'n T,
    pub dist: f32,
}

struct StackNode<'n, 'a, T> {
    nodes: Iter<'a, Node<'n, T>>,
}

impl<'n, 'a, T> StackNode<'n, 'a, T> {
    fn new_stack(root: Option<&'a Parent<'n, T>>, height: usize) -> Vec<StackNode<'n, 'a, T>> {
        let mut stack = Vec::with_capacity(height + 1);
        if let Some(root) = root {
            stack.push(StackNode {
                nodes: root.nodes.iter(),
            });
        }
        stack
    }
}

// scan iterator

pub struct ScanIterator<'n, 'a, T> {
    stack: Vec<StackNode<'n, 'a, T>>,
}

impl<'n, 'a, T> ScanIterator<'n, 'a, T> {
    fn new(root: Option<&'a Parent<'n, T>>, height: usize) -> Self {
        Self {
            stack: StackNode::new_stack(root, height),
        }
    }
}

impl<'n, 'a, T> Iterator for ScanIterator<'n, 'a, T> {
    type Item = IterItem<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        'outer: while let Some(stack) = self.stack.last_mut() {
            while let Some(node) = stack.nodes.next() {
                match node {
                    Node::Item(data) => {
                        return Some(IterItem {
                            rect: data.rect,
                            data: &data.item,
                            dist: Default::default(),
                        });
                    }
                    Node::Parent(nodes) => {
                        self.stack.push(StackNode {
                            nodes: nodes.nodes.iter(),
                        });
                        continue 'outer;
                    }
                }
            }
            self.stack.pop();
        }
        None
    }
}

// search iterator -- much like the scan iterator but with a intersects guard.

pub struct SearchIterator<'n, 'a, T> {
    stack: Vec<StackNode<'n, 'a, T>>,
    rect: Rect,
}

impl<'n, 'a, T> SearchIterator<'n, 'a, T> {
    fn new(root: Option<&'a Parent<'n, T>>, height: usize, rect: Rect) -> Self {
        Self {
            stack: StackNode::new_stack(root, height),
            rect,
        }
    }
}

impl<'n, 'a, T> Iterator for SearchIterator<'n, 'a, T> {
    type Item = IterItem<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        'outer: while let Some(stack) = self.stack.last_mut() {
            while let Some(node) = stack.nodes.next() {
                if !node.rect().intersects(&self.rect) {
                    continue;
                }
                match node {
                    Node::Item(data) => {
                        return Some(IterItem {
                            rect: data.rect,
                            data: &data.item,
                            dist: Default::default(),
                        });
                    }
                    Node::Parent(nodes) => {
                        self.stack.push(StackNode {
                            nodes: nodes.nodes.iter(),
                        });
                        continue 'outer;
                    }
                }
            }
            self.stack.pop();
        }
        None
    }
}

struct NearbyItem<'n, 'a, T> {
    dist: f32,
    node: &'a Node<'n, T>,
}

impl<'n, 'a, T> PartialEq for NearbyItem<'n, 'a, T> {
    fn eq(&self, other: &Self) -> bool {
        self.dist.eq(&other.dist)
    }
}

impl<'n, 'a, T> Eq for NearbyItem<'n, 'a, T> {}

impl<'n, 'a, T> PartialOrd for NearbyItem<'n, 'a, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.dist.partial_cmp(&other.dist).map(Ordering::reverse)
    }
}

impl<'n, 'a, T> Ord for NearbyItem<'n, 'a, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.dist.total_cmp(&other.dist)
    }
}

pub struct NearbyIterator<'n, 'a, T, F> {
    queue: BinaryHeap<NearbyItem<'n, 'a, T>>,
    dist: F,
}

impl<'n, 'a, T, F> NearbyIterator<'n, 'a, T, F>
where
    F: FnMut(&Rect, Option<&'a T>) -> f32,
{
    fn new(root: &'a Option<Node<'n, T>>, dist: F) -> Self {
        let mut queue = BinaryHeap::new();
        if let Some(root) = root {
            queue.push(NearbyItem {
                dist: Default::default(),
                node: root,
            });
        }
        NearbyIterator { queue, dist }
    }
}

impl<'n, 'a, T, F> Iterator for NearbyIterator<'n, 'a, T, F>
where
    F: FnMut(&Rect, Option<&'a T>) -> f32,
{
    type Item = IterItem<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some(item) = self.queue.pop() {
            match &item.node {
                Node::Item(data) => {
                    return Some(IterItem {
                        rect: data.rect,
                        data: &data.item,
                        dist: item.dist,
                    });
                }
                Node::Parent(nodes) => {
                    self.queue.extend(nodes.nodes.iter().map(|node| {
                        let (rect, item) = match node {
                            Node::Item(item) => (&item.rect, Some(&item.item)),
                            Node::Parent(nodes) => (&nodes.rect, None),
                        };
                        let dist = (self.dist)(rect, item);
                        NearbyItem { dist, node }
                    }));
                }
            }
        }
        None
    }
}

#[derive(PartialEq)]
struct Ordered(f32);

impl Eq for Ordered {}

impl PartialOrd for Ordered {
    fn partial_cmp(&self, other: &Ordered) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Ordered {
    fn cmp(&self, other: &Ordered) -> Ordering {
        if self.0 < other.0 {
            Ordering::Less
        } else if self.0 > other.0 {
            Ordering::Greater
        } else {
            Ordering::Equal
        }
    }
}

fn min(a: f32, b: f32) -> f32 {
    if a < b {
        a
    } else {
        b
    }
}

fn max(a: f32, b: f32) -> f32 {
    if a > b {
        a
    } else {
        b
    }
}
