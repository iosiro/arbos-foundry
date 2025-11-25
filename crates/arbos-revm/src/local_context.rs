use std::{cell::RefCell, collections::VecDeque, rc::Rc};

use revm::context::LocalContextTr;
use revm::primitives::B256;

pub trait ArbitrumLocalContextTr: LocalContextTr {
    fn stylus_pages_ever(&self) -> u16;
    fn stylus_pages_open(&self) -> u16;
    fn add_stylus_pages_open(&mut self, pages: u16);
    fn set_stylus_pages_open(&mut self, pages: u16);
    fn insert_recent_wasm(&mut self, code_hash: B256, retain: u16) -> bool;
}

/// Local context that is filled by execution.
#[derive(Clone, Debug)]
pub struct ArbitrumLocalContext {
    /// Interpreter shared memory buffer. A reused memory buffer for calls.
    pub shared_memory_buffer: Rc<RefCell<Vec<u8>>>,
    /// Stylus pages ever used in this transaction.
    pub stylus_pages_ever: u16,
    /// Stylus pages currently open.
    pub stylus_pages_open: u16,
    /// Recently invoked Stylus wasm code hashes (block-local LRU).
    pub recent_wasms: VecDeque<B256>,
}

impl Default for ArbitrumLocalContext {
    fn default() -> Self {
        Self {
            shared_memory_buffer: Rc::new(RefCell::new(Vec::with_capacity(1024 * 4))),
            stylus_pages_ever: 0,
            stylus_pages_open: 0,
            recent_wasms: VecDeque::new(),
        }
    }
}

impl LocalContextTr for ArbitrumLocalContext {
    fn clear(&mut self) {
        // Sets len to 0 but it will not shrink to drop the capacity.
        unsafe { self.shared_memory_buffer.borrow_mut().set_len(0) };
    }

    fn shared_memory_buffer(&self) -> &Rc<RefCell<Vec<u8>>> {
        &self.shared_memory_buffer
    }
}

impl ArbitrumLocalContextTr for ArbitrumLocalContext {
    fn stylus_pages_ever(&self) -> u16 {
        self.stylus_pages_ever
    }

    fn stylus_pages_open(&self) -> u16 {
        self.stylus_pages_open
    }

    fn add_stylus_pages_open(&mut self, pages: u16) {
        self.stylus_pages_open = self.stylus_pages_open.saturating_add(pages);
        if self.stylus_pages_open > self.stylus_pages_ever {
            self.stylus_pages_ever = self.stylus_pages_open;
        }
    }

    fn set_stylus_pages_open(&mut self, pages: u16) {
        self.stylus_pages_open = pages;
        if self.stylus_pages_open > self.stylus_pages_ever {
            self.stylus_pages_ever = self.stylus_pages_open;
        }
    }

    fn insert_recent_wasm(&mut self, code_hash: B256, retain: u16) -> bool {
        if let Some(pos) = self.recent_wasms.iter().position(|existing| *existing == code_hash) {
            // Move existing entry to the back to track recency.
            if pos + 1 != self.recent_wasms.len() {
                if let Some(found) = self.recent_wasms.remove(pos) {
                    self.recent_wasms.push_back(found);
                }
            }
            return true;
        }

        self.recent_wasms.push_back(code_hash);

        let retain = retain as usize;
        if retain > 0 && self.recent_wasms.len() > retain {
            self.recent_wasms.pop_front();
        }

        false
    }
}

impl ArbitrumLocalContext {
    /// Creates a new local context, initcodes are hashes and added to the mapping.
    pub fn new() -> Self {
        Self::default()
    }
}
