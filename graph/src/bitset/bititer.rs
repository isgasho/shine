use num_traits::{One, Zero};

use bitset::{BitBlock, BitSetLike, MAX_LEVEL};

/// Iterator over the set bits.
pub struct BitIter<'a, B: 'a + BitSetLike> {
    bitset: &'a B,
    // masked block for each level, consumed bits are cleared
    masks: [B::Bits; MAX_LEVEL],
    // index prefix for each level
    prefixes: [usize; MAX_LEVEL],
}

impl<'a, B: BitSetLike> BitIter<'a, B> {
    pub fn new<'b>(bitset: &'b B) -> BitIter<'b, B> {
        let mut iter = BitIter {
            bitset: bitset,
            masks: [B::Bits::zero(); MAX_LEVEL],
            prefixes: [0; MAX_LEVEL],
        };

        //init mask to perform a full descend when performing the first step
        let last_level = iter.bitset.get_level_count() - 1;
        let top = iter.bitset.get_block(last_level, 0);
        iter.masks[last_level] = top;
        iter
    }

    fn step(&mut self) -> Option<usize> {
        let lc = self.bitset.get_level_count();
        let mut level = 0;
        loop {
            while self.masks[level].is_zero() {
                // no bits in this block, move upward
                level = level + 1;
                if level >= lc {
                    // top reached and no more bits were found
                    return None;
                }
            }

            // take next set bit
            let prefix = {
                let block = &mut self.masks[level];
                let pos = block.trailing_bit_pos();
                *block = *block ^ (B::Bits::one() << pos);
                (self.prefixes[level] << B::Bits::bit_shift()) | pos
            };

            // move downward
            if level == 0 {
                // bottom reached, prefix is the index of the set bit
                return Some(prefix);
            }
            level = level - 1;
            self.masks[level] = self.bitset.get_block(level, prefix);
            self.prefixes[level] = prefix;
        }
    }
}

impl<'a, B: BitSetLike> Iterator for BitIter<'a, B> {
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        self.step()
    }
}
