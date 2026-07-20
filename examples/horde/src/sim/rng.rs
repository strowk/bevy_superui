use bevy::prelude::*;

#[derive(Resource, Clone)]
pub struct Rng {
    pub state: u64,
}

impl Rng {
    pub fn new(seed: u64) -> Self {
        // xorshift requires nonzero state.
        Rng { state: if seed == 0 { 0x9E3779B97F4A7C15 } else { seed } }
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x >> 12;
        x ^= x << 25;
        x ^= x >> 27;
        self.state = x;
        x.wrapping_mul(0x2545F491_4F6CDD1D)
    }

    /// Uniform in [0.0, 1.0). Uses the top 24 bits for an exact f32 mantissa.
    pub fn next_f32(&mut self) -> f32 {
        ((self.next_u64() >> 40) as f32) / ((1u32 << 24) as f32)
    }

    pub fn range(&mut self, lo: f32, hi: f32) -> f32 {
        lo + (hi - lo) * self.next_f32()
    }

    /// Uniformly-distributed unit vector.
    pub fn unit_vec(&mut self) -> Vec2 {
        let a = self.range(0.0, std::f32::consts::TAU);
        Vec2::new(a.cos(), a.sin())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_seed_same_sequence() {
        let mut a = Rng::new(42);
        let mut b = Rng::new(42);
        for _ in 0..1000 {
            assert_eq!(a.next_u64(), b.next_u64());
        }
    }

    #[test]
    fn f32_in_unit_range() {
        let mut r = Rng::new(7);
        for _ in 0..10_000 {
            let v = r.next_f32();
            assert!((0.0..1.0).contains(&v), "out of range: {v}");
        }
    }

    #[test]
    fn range_respects_bounds() {
        let mut r = Rng::new(9);
        for _ in 0..10_000 {
            let v = r.range(-3.0, 5.0);
            assert!((-3.0..=5.0).contains(&v));
        }
    }
}
