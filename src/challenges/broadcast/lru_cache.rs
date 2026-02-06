use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};

pub struct DoublyLinkedListNode {
    val: u64,
    next: Option<Rc<RefCell<DoublyLinkedListNode>>>,
    prev: Option<Weak<RefCell<DoublyLinkedListNode>>>,
}

pub struct DoublyLinkedList {
    head: Option<Rc<RefCell<DoublyLinkedListNode>>>,
    tail: Option<Rc<RefCell<DoublyLinkedListNode>>>,
}

impl DoublyLinkedList {
    pub fn new() -> Self {
        Self {
            head: None,
            tail: None,
        }
    }
}

pub struct LRUCache {
    size: u32,
    linked_list: DoublyLinkedList,
    node_hashmap: HashMap<u64, Rc<RefCell<DoublyLinkedListNode>>>,
}

impl LRUCache {
    pub fn new(size: u32) -> Self {
        Self {
            size,
            linked_list: DoublyLinkedList::new(),
            node_hashmap: HashMap::new(),
        }
    }

    pub fn add_val(&mut self, val: u64) -> () {
        if let Some(existing) = self.node_hashmap.get(&val) {
            let node = Rc::clone(existing);
            self.detach_node(&node);
            self.push_front(node);
            return;
        }

        let node = Rc::new(RefCell::new(DoublyLinkedListNode {
            val,
            next: None,
            prev: None,
        }));
        self.push_front(Rc::clone(&node));
        self.node_hashmap.insert(val, node);

        if self.node_hashmap.len() > self.size as usize {
            self.remove_last_used_val();
        }
    }

    pub fn remove_last_used_val(&mut self) -> () {
        if let Some(tail) = self.linked_list.tail.clone() {
            let val = tail.borrow().val;
            self.detach_node(&tail);
            self.node_hashmap.remove(&val);
        }
    }

    fn detach_node(&mut self, node: &Rc<RefCell<DoublyLinkedListNode>>) {
        let (prev, next) = {
            let borrowed = node.borrow();
            (
                borrowed.prev.clone().and_then(|w| w.upgrade()),
                borrowed.next.clone(),
            )
        };

        if let Some(prev_node) = prev.clone() {
            prev_node.borrow_mut().next = next.clone();
        } else {
            self.linked_list.head = next.clone();
        }

        if let Some(next_node) = next.clone() {
            next_node.borrow_mut().prev = prev.as_ref().map(Rc::downgrade);
        } else {
            self.linked_list.tail = prev.clone();
        }

        let mut borrowed = node.borrow_mut();
        borrowed.prev = None;
        borrowed.next = None;
    }

    fn push_front(&mut self, node: Rc<RefCell<DoublyLinkedListNode>>) {
        match self.linked_list.head.take() {
            Some(old_head) => {
                node.borrow_mut().next = Some(old_head.clone());
                node.borrow_mut().prev = None;
                old_head.borrow_mut().prev = Some(Rc::downgrade(&node));
                self.linked_list.head = Some(node);

                if self.linked_list.tail.is_none() {
                    self.linked_list.tail = Some(old_head);
                }
            }
            None => {
                self.linked_list.tail = Some(node.clone());
                self.linked_list.head = Some(node);
            }
        }
    }
}
