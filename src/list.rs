use std::collections::LinkedList;

pub struct VecDequeMax<T> {
    ls: LinkedList<T>,
    max: usize,
}

impl<T> VecDequeMax<T> {
    pub fn new(mut max: usize) -> Self {
        if max <= 0 {
            max = 20;
        }
        Self {
            ls: LinkedList::new(),
            max: max,
        }
    }
    pub fn len(&self) -> usize {
        self.ls.len()
    }
    pub fn push(&mut self, d: T) {
        self.ls.push_back(d);

        if self.len() > self.max {
            self.pop();
        }
    }
    pub fn pushf(&mut self, d: T) {
        self.ls.push_front(d);
    }
    pub fn pop(&mut self) -> Option<T> {
        self.ls.pop_front()
    }
    pub fn front(&self) -> Option<&T> {
        self.ls.front()
    }
}
