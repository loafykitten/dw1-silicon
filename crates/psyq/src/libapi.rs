//! libapi: the PS1 heap (`InitHeap3`, `malloc3`, `free3`).
//!
//! On the PS1, `initializeHeap()` (main.c) gives `InitHeap3` the RAM between the end of the
//! program and the stack: 0x30390 bytes (`_end` = 0x801BFB64 rounded up, stack top 0x801FFF00
//! minus 64 KB; read from SLUS_010.32). Natively the heap is a static arena inside the app's
//! data segment, so the GPU's 24-bit packet links can reach anything allocated here
//! (docs/phase2-plan.md). It's bigger than the PS1's, since native structs can be, and use
//! beyond the PS1's size is reported: the PS1 would have failed there.
//!
//! The allocator is first fit over the arena in address order, 16-byte aligned. Its addresses
//! don't need to match the PS1's; nothing in the game depends on them.

use std::collections::BTreeMap;
use std::ffi::c_void;
use std::sync::Mutex;

/// The PS1 heap's size (see the module docs).
pub const PS1_HEAP_SIZE: usize = 0x30390;

/// The native arena's size.
pub const ARENA_SIZE: usize = 4 << 20;

const ALIGN: usize = 16;

#[repr(C, align(16))]
struct Arena([u8; ARENA_SIZE]);

/// The arena itself. Only ever handed out as raw pointers to the game C.
static mut ARENA: Arena = Arena([0; ARENA_SIZE]);

/// Live allocations: offset into the arena -> size (rounded up to `ALIGN`).
struct Heap {
    blocks: BTreeMap<usize, usize>,
    in_use: usize,
    warned: bool,
}

static HEAP: Mutex<Heap> = Mutex::new(Heap {
    blocks: BTreeMap::new(),
    in_use: 0,
    warned: false,
});

fn arena_base() -> *mut u8 {
    (&raw mut ARENA).cast()
}

impl Heap {
    /// First fit: the lowest gap between live blocks that holds `size` bytes.
    fn alloc(&mut self, size: usize) -> Option<usize> {
        let size = size.max(1).next_multiple_of(ALIGN);
        let mut start = 0;
        for (&offset, &len) in &self.blocks {
            if offset - start >= size {
                break;
            }
            start = offset + len;
        }
        if start + size > ARENA_SIZE {
            return None;
        }
        self.blocks.insert(start, size);
        self.in_use += size;
        if self.in_use > PS1_HEAP_SIZE && !self.warned {
            self.warned = true;
            eprintln!(
                "psyq: heap use {:#x} passed the PS1's {PS1_HEAP_SIZE:#x} bytes (malloc3 would \
                 have failed on the PS1)",
                self.in_use
            );
        }
        Some(start)
    }

    fn free(&mut self, offset: usize) -> bool {
        match self.blocks.remove(&offset) {
            Some(len) => {
                self.in_use -= len;
                true
            }
            None => false,
        }
    }

    fn reset(&mut self) {
        self.blocks.clear();
        self.in_use = 0;
    }
}

/// `InitHeap3(head, size)`: (re)starts the heap empty. The PS1 region it names isn't used;
/// the native arena stands in for it.
#[unsafe(no_mangle)]
pub extern "C" fn InitHeap3(_head: *mut u32, _size: u32) {
    HEAP.lock().unwrap().reset();
}

/// `malloc3(size)`: a block of at least `size` bytes, or NULL when the arena is full.
#[unsafe(no_mangle)]
pub extern "C" fn malloc3(size: u32) -> *mut c_void {
    match HEAP.lock().unwrap().alloc(size as usize) {
        Some(offset) => arena_base().wrapping_add(offset).cast(),
        None => std::ptr::null_mut(),
    }
}

/// `free3(block)`: frees a block from `malloc3`; NULL does nothing. Anything else is a game
/// bug that would corrupt the PS1's heap, so it stops the program.
#[unsafe(no_mangle)]
pub extern "C" fn free3(block: *mut c_void) {
    if block.is_null() {
        return;
    }
    let offset = (block as usize).wrapping_sub(arena_base() as usize);
    if offset >= ARENA_SIZE || !HEAP.lock().unwrap().free(offset) {
        crate::trap::stop(&format!("psyq: free3({block:p}) isn't a live malloc3 block"));
    }
}

/// Bytes currently allocated (rounded to the allocator's alignment).
pub fn heap_in_use() -> usize {
    HEAP.lock().unwrap().in_use
}

/// Whether `ptr..ptr+len` lies inside the heap arena.
pub fn in_arena(ptr: *const u8, len: usize) -> bool {
    let base = arena_base() as usize;
    let p = ptr as usize;
    p >= base
        && p.checked_add(len)
            .is_some_and(|end| end <= base + ARENA_SIZE)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_fit_reuses_freed_gaps() {
        let mut heap = Heap {
            blocks: BTreeMap::new(),
            in_use: 0,
            warned: true,
        };
        let a = heap.alloc(10).unwrap();
        let b = heap.alloc(40).unwrap();
        let c = heap.alloc(16).unwrap();
        assert_eq!((a, b, c), (0, 16, 64));
        assert!(heap.free(b));
        assert!(!heap.free(b));
        // A small block fits in the freed gap, a big one goes after the last block.
        assert_eq!(heap.alloc(20), Some(16));
        assert_eq!(heap.alloc(100), Some(80));
        assert_eq!(heap.alloc(16), Some(48));
        assert_eq!(heap.in_use, 16 + 32 + 16 + 112 + 16);
        assert_eq!(heap.alloc(ARENA_SIZE), None);
        heap.reset();
        assert_eq!(heap.alloc(ARENA_SIZE), Some(0));
    }

    #[test]
    fn the_c_interface_hands_out_arena_memory() {
        // The only test touching the global heap.
        InitHeap3(std::ptr::null_mut(), 0);
        let p = malloc3(100);
        assert!(!p.is_null());
        assert_eq!(p as usize % ALIGN, 0);
        assert!(in_arena(p.cast(), 100));
        assert_eq!(heap_in_use(), 112);
        free3(p);
        free3(std::ptr::null_mut());
        assert_eq!(heap_in_use(), 0);
    }
}
