#[cfg(test)]
mod test;

use arrayvec::ArrayVec;
use std::cmp::Ordering;
use std::collections::BinaryHeap;
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

type NodeVec<T> = ArrayVec<Node<T>, MAX_ITEMS>;

#[derive(Clone)]
struct Nodes<T> {
    nodes: Box<NodeVec<T>>,
}

impl<T> Nodes<T> {
    fn new(rect: Rect) -> Self {
        Self {
            rect,
            nodes: Box::new(NodeVec::new()),
        }
    }

    fn is_full(&self) -> bool {
        self.nodes.is_full()
    }

    fn len(&self) -> usize {
        self.nodes.len()
    }

    fn choose_least_enlargement(&mut self, rect: &Rect) -> Option<&mut Nodes<T>> {
        let mut res = None;
        let mut min_enlargement = rect.min.x;
        let mut min_area = rect.min.x;
        for node in self.nodes.iter_mut() {
            let uarea = node.rect.unioned_area(rect);
            let area = node.rect.area();
            let enlargement = uarea - area;
            if res.is_none()
                || enlargement < min_enlargement
                || (enlargement == min_enlargement && area < min_area)
            {
                res = Some(node);
                min_enlargement = enlargement;
                min_area = area;
            }
        }
        res
    }

    fn insert(&mut self, rect: Rect, item: T, height: usize) {
        if height == 0 {
            // leaf node
            self.nodes.push(Node::item(rect, item));
        } else {
            // branch node
            if let Some(Node {
                data: Data::Nodes(child),
                ..
            }) = self.choose_least_enlargement(&rect)
            {
                child.insert(rect, item, height - 1);
                if child.is_full() {
                    let right = child.split_largest_axis_edge_snap();
                    self.nodes.push(right);
                }
            }
        }
        self.rect.expand(&rect);
    }

    fn recalc(&mut self) {
        if self.nodes.len() == 0 {
            return;
        }
        let mut rect = self.nodes[0].rect.clone();
        for i in 1..self.nodes.len() {
            rect.expand(&self.nodes[i].rect);
        }
        self.rect = rect;
    }

    fn split_largest_axis_edge_snap(&mut self) -> Node<T> {
        let rect = self.rect;
        let axis = rect.largest_axis();
        let mut right = Nodes::new(rect);
        let lchilds = &mut self.nodes;
        let rchilds = &mut right.nodes;
        let mut i = 0;
        while i < lchilds.len() {
            let min = lchilds[i].rect.min.on(axis) - rect.min.on(axis);
            let max = rect.max.on(axis) - lchilds[i].rect.max.on(axis);
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
            rchilds.sort_unstable_by_key(|n| Ordered(n.rect.min.on(axis)));
            while lchilds.len() < MIN_ITEMS {
                lchilds.push(rchilds.pop().unwrap());
            }
        } else if rchilds.len() < MIN_ITEMS {
            // reverse sort by max axis
            lchilds.sort_unstable_by_key(|n| Ordered(n.rect.max.on(axis)));
            while rchilds.len() < MIN_ITEMS {
                rchilds.push(lchilds.pop().unwrap());
            }
        }
        // recalculate and sort the nodes
        self.recalc();
        right.recalc();
        self.sort_by_x();
        right.sort_by_x();
        Node::Nodes(right)
    }

    fn push(&mut self, child: Node<T>) {
        self.nodes.push(child);
    }

    fn sort_by_x(&mut self) {
        self.nodes.sort_unstable_by_key(|n| Ordered(n.rect.min.x));
    }

    fn flatten_into(&mut self, reinsert: &mut Vec<(Rect, T)>) {
        let nodes = &mut self.nodes;
        while let Some(node) = nodes.pop() {
            match node {
                Node::Item(item) => reinsert.push((item.rect, item.item)),
                Node::Nodes(mut nodes) => nodes.flatten_into(reinsert),
            }
        }
    }

    pub fn remove(
        &mut self,
        rect: &Rect,
        data: &T,
        reinsert: &mut Vec<(Rect, T)>,
        height: usize,
    ) -> (Option<(Rect, T)>, bool)
    where
        T: PartialEq,
    {
        let nodes = &mut self.nodes;
        if height == 0 {
            // remove from leaf
            for i in 0..nodes.len() {
                if nodes[i].item() == data {
                    let out = nodes.swap_remove(i);
                    let recalced = self.rect.on_edge(&out.rect);
                    if recalced {
                        self.recalc();
                    }
                    return (Some((out.rect.clone(), out.into_item())), recalced);
                }
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

    pub fn search_flat<'a>(&'a self, rect: &Rect, items: &mut Vec<(Rect, &'a T)>) {
        for node in self.nodes.iter() {
            if node.rect.intersects(&rect) {
                match node {
                    Node::Item(item) => items.push((item.rect, &item.item)),
                    Node::Nodes(nodes) => nodes.search_flat(&rect, items),
                }
            }
        }
    }
}

#[derive(Clone)]
enum Data<T> {
    Item(T),
    Nodes(Nodes<T>),
}

#[derive(Clone)]
struct Node<T> {
    rect: Rect,
    data: Data<T>,
}

impl<T> Node<T> {
    fn item(rect: Rect, item: T) -> Self {
        Self {
            rect,
            data: Data::Item(item),
        }
    }
}

#[derive(Clone)]
pub struct RTree<T> {
    root: Option<Node<T>>,
    length: usize,
    height: usize,
}

impl<T: PartialEq> RTree<T> {
    pub fn new() -> Self {
        RTree {
            root: None,
            length: 0,
            height: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.length
    }

    pub fn rect(&self) -> Option<Rect> {
        self.root.as_ref().map(|root| root.rect.clone())
    }

    pub fn insert(&mut self, rect: Rect, data: T) {
        let root = self
            .root
            .get_or_insert_with(|| Node::Nodes(Nodes::new(rect)))
            .nodes_mut();
        root.insert(rect, data, self.height);
        if root.is_full() {
            let mut new_root = Nodes::new(root.rect);
            let right = root.split_largest_axis_edge_snap();
            let left = self.root.take().unwrap();
            new_root.push(left);
            new_root.push(right);
            self.root = Some(Node::Nodes(new_root));
            self.height += 1;
        }
        self.length += 1;
    }

    pub fn remove(&mut self, rect: Rect, data: &T) -> Option<(Rect, T)> {
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
                self.insert(item.0, item.1);
            }
            removed
        } else {
            None
        }
    }

    pub fn search_flat<'a>(&'a self, rect: Rect, items: &mut Vec<(Rect, &'a T)>) {
        if let Some(root) = &self.root {
            root.nodes().search_flat(&rect, items);
        }
    }

    pub fn iter(&self) -> ScanIterator<T> {
        self.scan()
    }

    pub fn scan(&self) -> ScanIterator<T> {
        ScanIterator::new(self.root.as_ref().map(Node::nodes), self.height)
    }

    pub fn search(&self, rect: Rect) -> SearchIterator<T> {
        SearchIterator::new(self.root.as_ref().map(Node::nodes), self.height, rect)
    }

    pub fn nearby<F>(&self, dist: F) -> NearbyIterator<T, F>
    where
        F: FnMut(&Rect, Option<&'_ T>) -> f32,
    {
        NearbyIterator::new(&self.root, dist)
    }
}

// iterators, ScanIterator, SearchIterator, NearbyIterator

pub struct IterItem<'a, T> {
    pub rect: Rect,
    pub data: &'a T,
    pub dist: f32,
}

struct StackNode<'a, T> {
    nodes: Iter<'a, Node<T>>,
}

impl<'a, T> StackNode<'a, T> {
    fn new_stack(root: Option<&'a Nodes<T>>, height: usize) -> Vec<StackNode<'a, T>> {
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

pub struct ScanIterator<'a, T> {
    stack: Vec<StackNode<'a, T>>,
}

impl<'a, T> ScanIterator<'a, T> {
    fn new(root: Option<&'a Nodes<T>>, height: usize) -> Self {
        Self {
            stack: StackNode::new_stack(root, height),
        }
    }
}

impl<'a, T> Iterator for ScanIterator<'a, T> {
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
                    Node::Nodes(nodes) => {
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

pub struct SearchIterator<'a, T> {
    stack: Vec<StackNode<'a, T>>,
    rect: Rect,
}

impl<'a, T> SearchIterator<'a, T> {
    fn new(root: Option<&'a Nodes<T>>, height: usize, rect: Rect) -> Self {
        Self {
            stack: StackNode::new_stack(root, height),
            rect,
        }
    }
}

impl<'a, T> Iterator for SearchIterator<'a, T> {
    type Item = IterItem<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        'outer: while let Some(stack) = self.stack.last_mut() {
            while let Some(node) = stack.nodes.next() {
                if !node.rect.intersects(&self.rect) {
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
                    Node::Nodes(nodes) => {
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

struct NearbyItem<'a, T> {
    dist: f32,
    node: &'a Node<T>,
}

impl<'a, T> PartialEq for NearbyItem<'a, T> {
    fn eq(&self, other: &Self) -> bool {
        self.dist.eq(&other.dist)
    }
}

impl<'a, T> Eq for NearbyItem<'a, T> {}

impl<'a, T> PartialOrd for NearbyItem<'a, T> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        self.dist.partial_cmp(&other.dist).map(Ordering::reverse)
    }
}

impl<'a, T> Ord for NearbyItem<'a, T> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.dist.total_cmp(&other.dist)
    }
}

pub struct NearbyIterator<'a, T, F> {
    queue: BinaryHeap<NearbyItem<'a, T>>,
    dist: F,
}

impl<'a, T, F> NearbyIterator<'a, T, F>
where
    F: FnMut(&Rect, Option<&'a T>) -> f32,
{
    fn new(root: &'a Option<Node<T>>, dist: F) -> Self {
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

impl<'a, T, F> Iterator for NearbyIterator<'a, T, F>
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
                Node::Nodes(nodes) => {
                    self.queue.extend(nodes.nodes.iter().map(|node| {
                        let (rect, item) = match node {
                            Node::Item(item) => (&item.rect, Some(&item.item)),
                            Node::Nodes(nodes) => (&nodes.rect, None),
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
