/// Xor-shift 乱数生成アルゴリズムにより乱数を生成します．
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

    /// 内部状態を 1 ステップ進め，乱数を返します．
    pub fn next(&mut self) -> u32 {
        let t = self.x ^ (self.x << 11);
        self.x = self.y;
        self.y = self.z;
        self.z = self.w;
        self.w = (self.w ^ (self.w >> 19)) ^ (t ^ (t >> 8));
        self.w & 0x7FFFFFFF
    }
}
