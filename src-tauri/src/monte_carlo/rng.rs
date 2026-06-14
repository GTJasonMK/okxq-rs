/// 简单的 xorshift64 伪随机数生成器（无需 rand crate）
pub(super) struct XorShift64 {
    state: u64,
}

impl XorShift64 {
    pub(super) fn new(seed: u64) -> Self {
        Self {
            state: seed.wrapping_add(0x9E3779B97F4A7C15),
        }
    }

    pub(super) fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
}
