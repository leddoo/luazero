use core::ptr::NonNull;
use core::marker::PhantomData;

use sti::arena::Arena;
use sti::keyed::Key;


pub struct BitSetImpl<'a, K: Key, const MUT: bool> {
    // invariant: bits[self.len..] = 0
    ptr: NonNull<u32>,
    len: usize,
    phantom: PhantomData<(&'a u32, fn(K) -> bool)>,
}

pub type BitSet<'a, K>    = BitSetImpl<'a, K, false>;
pub type BitSetMut<'a, K> = BitSetImpl<'a, K, true>;


impl<'a, K: Key> BitSetMut<'a, K> {
    pub fn new(alloc: &'a Arena, len: usize) -> BitSetMut<'a, K> {
        assert!(K::from_usize(len).is_some());

        let size = sti::num::ceil_to_multiple_pow2(len, 32);
        let ptr = sti::alloc::alloc_array(alloc, size).unwrap();
        unsafe { core::ptr::write_bytes(ptr.as_ptr(), 0, size) };
        BitSetMut { ptr, len, phantom: PhantomData }
    }

    #[track_caller]
    #[inline(always)]
    pub fn insert(&mut self, k: K) {
        let k = k.usize();
        assert!(k < self.len);

        let idx = k / 32;
        let bit = k % 32;
        unsafe { *self.ptr.as_ptr().add(idx) |= 1 << bit; }
    }

    #[track_caller]
    #[inline(always)]
    pub fn remove(&mut self, k: K) {
        let k = k.usize();
        assert!(k < self.len);

        let idx = k / 32;
        let bit = k % 32;
        unsafe { *self.ptr.as_ptr().add(idx) &= !(1 << bit); }
    }

    #[track_caller]
    #[inline(always)]
    pub fn union<const MUT: bool>(&mut self, other: &BitSetImpl<'a, K, MUT>) {
        assert_eq!(self.len, other.len);
        for i in 0..self.len { unsafe {
            *self.ptr.as_ptr().add(i) |= other.ptr.as_ptr().add(i).read()
        }}
    }

    #[track_caller]
    #[inline(always)]
    pub fn intersect<const MUT: bool>(&mut self, other: &BitSetImpl<'a, K, MUT>) {
        assert_eq!(self.len, other.len);
        for i in 0..self.len { unsafe {
            *self.ptr.as_ptr().add(i) &= other.ptr.as_ptr().add(i).read()
        }}
    }

    #[track_caller]
    #[inline(always)]
    pub fn minus<const MUT: bool>(&mut self, other: &BitSetImpl<'a, K, MUT>) {
        assert_eq!(self.len, other.len);
        for i in 0..self.len { unsafe {
            *self.ptr.as_ptr().add(i) &= !other.ptr.as_ptr().add(i).read()
        }}
    }
}

impl<'a, K: Key, const MUT: bool> BitSetImpl<'a, K, MUT> {
    #[inline(always)]
    pub fn borrow<'this>(&'this self) -> BitSet<'this, K> {
        BitSet { ptr: self.ptr, len: self.len, phantom: PhantomData }
    }

    #[track_caller]
    #[inline(always)]
    pub fn has(&self, k: K) -> bool {
        let k = k.usize();
        assert!(k < self.len);

        let idx = k / 32;
        let bit = k % 32;
        let word = unsafe { self.ptr.as_ptr().add(idx).read() };
        (word & (1 << bit)) != 0
    }

    #[track_caller]
    #[inline(always)]
    pub fn iter(&self) -> BitSetIter<K> {
        let size = sti::num::ceil_to_multiple_pow2(self.len, 32);
        BitSetIter { set: self.borrow(), size, idx: 0, buffer: 0 }
    }
}


impl<'a, K: Key> Clone for BitSet<'a, K> {
    #[inline(always)]
    fn clone(&self) -> Self { *self }
}

impl<'a, K: Key> Copy for BitSet<'a, K> {}


#[must_use = "iterators are lazy and do nothing unless consumed"]
pub struct BitSetIter<'a, K: Key> {
    set: BitSet<'a, K>,
    size: usize,
    idx:  usize,
    buffer: u32,
}

impl<'a, K: Key> Iterator for BitSetIter<'a, K> {
    type Item = K;

    #[inline(always)]
    fn next(&mut self) -> Option<Self::Item> {
        while self.buffer == 0 {
            if self.idx >= self.size {
                return None;
            }

            self.buffer = unsafe { self.set.ptr.as_ptr().add(self.idx).read() };
            self.idx += 1;
        }

        let bit = self.buffer.trailing_zeros();
        self.buffer &= self.buffer - 1;

        Some(K::from_usize_unck(32*(self.idx - 1) + bit as usize))
    }

    #[inline(always)]
    fn size_hint(&self) -> (usize, Option<usize>) {
        let len = self.set.len - self.idx;
        (0, Some(len))
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitset_basic() {
        let alloc = Arena::new();

        sti::define_key!(u32, K);

        fn k(i: usize) -> K { K::from_usize(i).unwrap() }

        let mut a = BitSetMut::new(&alloc, 72);
        for i in 0..72 {
            assert_eq!(a.has(k(i)), false);
        }
        assert_eq!(a.iter().next(), None);

        assert_eq!(a.has(k(31)), false);
        a.insert(k(31));
        assert_eq!(a.has(k(31)), true);
        a.remove(k(31));
        assert_eq!(a.has(k(31)), false);

        a.insert(k(31));
        a.insert(k(32));
        a.insert(k(71));
        assert_eq!(a.has(k(31)), true);
        assert_eq!(a.has(k(32)), true);
        assert_eq!(a.has(k(71)), true);

        let mut it = a.iter();
        assert_eq!(it.next(), Some(k(31)));
        assert_eq!(it.next(), Some(k(32)));
        assert_eq!(it.next(), Some(k(71)));
        assert_eq!(it.next(), None);
    }
}

