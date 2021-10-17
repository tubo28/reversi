/// Stateful random number generator by XorShift algorithm.
pub struct Xor128 {
    x: u32,
    y: u32,
    z: u32,
    w: u32,
}

impl Xor128 {
    pub fn from_seed(seed: u32) -> Xor128 {
        let mut res = Xor128 { x: 123456789, y: 987654321, z: 1000000007, w: seed };
        for _ in 0..16 {
            res.next();
        }
        res
    }

    /// Proceed the state by one step and returns a random number less than 2^31-1.
    pub fn next(&mut self) -> u32 {
        let t = self.x ^ (self.x << 11);
        self.x = self.y;
        self.y = self.z;
        self.z = self.w;
        self.w = (self.w ^ (self.w >> 19)) ^ (t ^ (t >> 8));
        self.w & 0x7FFFFFFF
    }
}
