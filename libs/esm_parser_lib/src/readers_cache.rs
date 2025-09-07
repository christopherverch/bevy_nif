use std::cell::{Ref, RefCell, RefMut};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};

use crate::{EsmError, EsmReader};

// Assuming these structs are defined elsewhere
// use crate::esmreader::{EsmReader, EsmError};
// use crate::header_types::Header;
// use crate::common_types::EsmContext;

#[derive(Debug, PartialEq, Eq, Copy, Clone)]
enum ReaderState {
    Busy,
    Free,
    Closed,
}

struct CacheItem {
    state: ReaderState,
    reader: EsmReader,
    name: Option<PathBuf>,
}

pub struct BusyItem {
    owner: Rc<RefCell<ReadersCache>>,
    item: Rc<RefCell<CacheItem>>,
}

impl Drop for BusyItem {
    fn drop(&mut self) {
        let mut cache = self.owner.borrow_mut();
        cache.release_item(self.item.clone());
    }
}

impl BusyItem {
    pub fn borrow(&self) -> Ref<EsmReader> {
        Ref::map(self.item.borrow(), |item| &item.reader)
    }

    pub fn borrow_mut(&self) -> RefMut<EsmReader> {
        RefMut::map(self.item.borrow_mut(), |item| &mut item.reader)
    }
}

pub struct ReadersCache {
    capacity: usize,
    items: Vec<Rc<RefCell<CacheItem>>>,
    index: HashMap<usize, Weak<RefCell<CacheItem>>>,
    busy_indices: HashMap<usize, ()>,
    free_queue: VecDeque<usize>,
    closed_queue: VecDeque<usize>,
}

impl ReadersCache {
    pub fn new(capacity: usize) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(ReadersCache {
            capacity,
            items: Vec::new(),
            index: HashMap::new(),
            busy_indices: HashMap::new(),
            free_queue: VecDeque::new(),
            closed_queue: VecDeque::new(),
        }))
    }

    fn release_item(&mut self, item: Rc<RefCell<CacheItem>>) {
        let index = self
            .get_index_for_item(&item)
            .expect("Released item not in cache");
        self.busy_indices.remove(&index);

        let mut item_mut = item.borrow_mut();
        item_mut.reader.close();
        item_mut.state = ReaderState::Free;
        self.free_queue.push_back(index);
    }
    pub fn get(
        cache_rc: Rc<RefCell<Self>>,
        index: usize,
        path: &Path,
    ) -> Result<BusyItem, EsmError> {
        let mut self_mut = cache_rc.borrow_mut();

        if self_mut.busy_indices.contains_key(&index) {
            return Err(EsmError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "ESMReader is busy",
            )));
        }

        let item_rc = if let Some(weak_item) = self_mut.index.get(&index) {
            let item_rc = weak_item
                .upgrade()
                .expect("Weak reference to CacheItem is gone");
            let mut item_mut = item_rc.borrow_mut();
            match item_mut.state {
                ReaderState::Free => {
                    item_mut.state = ReaderState::Busy;
                    // Move the index from free to busy
                    self_mut.free_queue.retain(|&x| x != index);
                    item_rc.clone()
                }
                ReaderState::Closed => {
                    item_mut.reader.open_raw(path)?;
                    item_mut.state = ReaderState::Busy;
                    // Move the index from closed to busy
                    self_mut.closed_queue.retain(|&x| x != index);
                    item_rc.clone()
                }
                ReaderState::Busy => {
                    // Should be caught by the initial check, but good for safety
                    unreachable!("Item should not be busy here");
                }
            }
        } else {
            self_mut.close_extra_readers();
            let mut reader = EsmReader::new()?;
            reader.open_raw(path)?;
            let new_item = Rc::new(RefCell::new(CacheItem {
                state: ReaderState::Busy,
                reader,
                name: Some(path.to_path_buf()),
            }));
            self_mut.items.push(new_item.clone());
            self_mut.index.insert(index, Rc::downgrade(&new_item));
            new_item
        };

        self_mut.busy_indices.insert(index, ());
        drop(self_mut); // Explicitly drop the mutable borrow

        Ok(BusyItem {
            owner: Rc::clone(&cache_rc),
            item: item_rc,
        })
    }
    pub fn get_name(&self, index: usize) -> Result<PathBuf, EsmError> {
        let weak_item = self.index.get(&index).ok_or_else(|| {
            EsmError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Reader not in cache",
            ))
        })?;
        let item = weak_item.upgrade().ok_or_else(|| {
            EsmError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Reader is no longer in cache",
            ))
        })?;
        Ok(item.borrow().name.clone().unwrap())
    }

    pub fn get_file_size(&self, index: usize) -> Result<usize, EsmError> {
        let weak_item = self.index.get(&index).ok_or_else(|| {
            EsmError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Reader not in cache",
            ))
        })?;
        let item = weak_item.upgrade().ok_or_else(|| {
            EsmError::Io(std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "Reader is no longer in cache",
            ))
        })?;
        Ok(item.borrow().reader.get_file_size())
    }

    pub fn clear(&mut self) {
        self.items.clear();
        self.index.clear();
        self.busy_indices.clear();
        self.free_queue.clear();
        self.closed_queue.clear();
    }

    fn close_extra_readers(&mut self) {
        while self.free_queue.len() + self.busy_indices.len() + 1 > self.capacity {
            if let Some(index) = self.free_queue.pop_front() {
                let item_rc = self
                    .index
                    .get(&index)
                    .and_then(|weak| weak.upgrade())
                    .expect("Item in free queue not found");
                let mut item_mut = item_rc.borrow_mut();
                item_mut.reader.close();
                item_mut.state = ReaderState::Closed;
                self.closed_queue.push_back(index);
            } else {
                break;
            }
        }
    }

    // A helper method to find the index for a given item.
    fn get_index_for_item(&self, item_rc: &Rc<RefCell<CacheItem>>) -> Option<usize> {
        for (i, existing_item) in self.items.iter().enumerate() {
            if Rc::ptr_eq(existing_item, item_rc) {
                return Some(i);
            }
        }
        None
    }
}

