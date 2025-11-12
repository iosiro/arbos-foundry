use std::{cell::RefCell, rc::Rc};

use revm::context::LocalContextTr;

pub trait ArbitrumLocalContextTr: LocalContextTr {
    fn stylus_pages_ever(&self) -> u64;
    fn stylus_pages_open(&self) -> u64;
    fn add_stylus_pages_open(&mut self, pages: u64);
    fn set_stylus_pages_open(&mut self, pages: u64);
}

/// Local context that is filled by execution.
#[derive(Clone, Debug)]
pub struct ArbitrumLocalContext {
    /// Interpreter shared memory buffer. A reused memory buffer for calls.
    pub shared_memory_buffer: Rc<RefCell<Vec<u8>>>,
    /// Stylus pages ever used in this transaction.
    pub stylus_pages_ever: u64,
    /// Stylus pages currently open.
    pub stylus_pages_open: u64,
}

impl Default for ArbitrumLocalContext {
    fn default() -> Self {
        Self {
            shared_memory_buffer: Rc::new(RefCell::new(Vec::with_capacity(1024 * 4))),
            stylus_pages_ever: 0,
            stylus_pages_open: 0,
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
    fn stylus_pages_ever(&self) -> u64 {
        self.stylus_pages_ever
    }

    fn stylus_pages_open(&self) -> u64 {
        self.stylus_pages_open
    }

    fn add_stylus_pages_open(&mut self, pages: u64) {
        self.stylus_pages_open = self.stylus_pages_open.saturating_add(pages);
        if self.stylus_pages_open > self.stylus_pages_ever {
            self.stylus_pages_ever = self.stylus_pages_open;
        }
    }

    fn set_stylus_pages_open(&mut self, pages: u64) {
        self.stylus_pages_open = pages;
        if self.stylus_pages_open > self.stylus_pages_ever {
            self.stylus_pages_ever = self.stylus_pages_open;
        }
    }
}

impl ArbitrumLocalContext {
    /// Creates a new local context, initcodes are hashes and added to the mapping.
    pub fn new() -> Self {
        Self::default()
    }
}