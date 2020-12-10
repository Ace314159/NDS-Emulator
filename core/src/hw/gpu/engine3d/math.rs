use std::ops::{AddAssign, Add, Mul, Neg, Index};

#[derive(Clone, Copy, Debug)]
pub struct Matrix {
    elems: [FixedPoint; 16],
}

impl Matrix {
    pub fn identity() -> Self {
        Matrix {
            elems: [
                FixedPoint::one(), FixedPoint::zero(), FixedPoint::zero(), FixedPoint::zero(),
                FixedPoint::zero(), FixedPoint::one(), FixedPoint::zero(), FixedPoint::zero(),
                FixedPoint::zero(), FixedPoint::zero(), FixedPoint::one(), FixedPoint::zero(),
                FixedPoint::zero(), FixedPoint::zero(), FixedPoint::zero(), FixedPoint::one(),
            ],
        }
    }

    pub fn set_identity(&mut self, _params: &Vec<u32>) { *self = Matrix::identity() }

    pub fn load4x4(&mut self, vec: &Vec<u32>) {
        assert_eq!(vec.len(), 16);
        for i in 0..16 {
            self.elems[i] = FixedPoint(vec[i] as i32);
        }
    }

    pub fn load4x3(&mut self, vec: &Vec<u32>) {
        assert_eq!(vec.len(), 12);
        for row in 0..4 {
            for col in 0..3 { self.elems[row * 4 + col] = FixedPoint(vec[row * 3 + col] as i32) }
            self.elems[row * 4 + 3] = FixedPoint::zero(); 
        }
        self.elems[15] = FixedPoint::one();
    }

    pub fn mul4x4(&mut self, vec: &Vec<u32>) {
        assert_eq!(vec.len(), 16);
        let old = self.elems.clone();
        // self = vec * self
        self.elems[0] = FixedPoint::from_mul(vec[0] * old[0] + vec[1] * old[4] + vec[2] * old[8] + vec[3] * old[12]);
        self.elems[1] = FixedPoint::from_mul(vec[0] * old[1] + vec[1] * old[5] + vec[2] * old[9] + vec[3] * old[13]);
        self.elems[2] = FixedPoint::from_mul(vec[0] * old[2] + vec[1] * old[6] + vec[2] * old[10] + vec[3] * old[14]);
        self.elems[3] = FixedPoint::from_mul(vec[0] * old[3] + vec[1] * old[7] + vec[2] * old[11] + vec[3] * old[15]);

        self.elems[4] = FixedPoint::from_mul(vec[4] * old[0] + vec[5] * old[4] + vec[6] * old[8] + vec[7] * old[12]);
        self.elems[5] = FixedPoint::from_mul(vec[4] * old[1] + vec[5] * old[5] + vec[6] * old[9] + vec[7] * old[13]);
        self.elems[6] = FixedPoint::from_mul(vec[4] * old[2] + vec[5] * old[6] + vec[6] * old[10] + vec[7] * old[14]);
        self.elems[7] = FixedPoint::from_mul(vec[4] * old[3] + vec[5] * old[7] + vec[6] * old[11] + vec[7] * old[15]);

        self.elems[8] = FixedPoint::from_mul(vec[8] * old[0] + vec[9] * old[4] + vec[10] * old[8] + vec[11] * old[12]);
        self.elems[9] = FixedPoint::from_mul(vec[8] * old[1] + vec[9] * old[5] + vec[10] * old[9] + vec[11] * old[13]);
        self.elems[10] = FixedPoint::from_mul(vec[8] * old[2] + vec[9] * old[6] + vec[10] * old[10] + vec[11] * old[14]);
        self.elems[11] = FixedPoint::from_mul(vec[8] * old[3] + vec[9] * old[7] + vec[10] * old[11] + vec[11] * old[15]);

        self.elems[12] = FixedPoint::from_mul(vec[12] * old[0] + vec[13] * old[4] + vec[14] * old[8] + vec[15] * old[12]);
        self.elems[13] = FixedPoint::from_mul(vec[12] * old[1] + vec[13] * old[5] + vec[14] * old[9] + vec[15] * old[13]);
        self.elems[14] = FixedPoint::from_mul(vec[12] * old[2] + vec[13] * old[6] + vec[14] * old[10] + vec[15] * old[14]);
        self.elems[15] = FixedPoint::from_mul(vec[12] * old[3] + vec[13] * old[7] + vec[14] * old[11] + vec[15] * old[15]);
    }

    pub fn mul4x3(&mut self, vec: &Vec<u32>) {
        assert_eq!(vec.len(), 12);
        let old = self.elems.clone();
        // self = vec * self
        self.elems[0] = FixedPoint::from_mul(vec[0] * old[0] + vec[1] * old[4] + vec[2] * old[8]);
        self.elems[1] = FixedPoint::from_mul(vec[0] * old[1] + vec[1] * old[5] + vec[2] * old[9]);
        self.elems[2] = FixedPoint::from_mul(vec[0] * old[2] + vec[1] * old[6] + vec[2] * old[10]);
        self.elems[3] = FixedPoint::from_mul(vec[0] * old[3] + vec[1] * old[7] + vec[2] * old[11]);

        self.elems[4] = FixedPoint::from_mul(vec[3] * old[0] + vec[4] * old[4] + vec[5] * old[8]);
        self.elems[5] = FixedPoint::from_mul(vec[3] * old[1] + vec[4] * old[5] + vec[5] * old[9]);
        self.elems[6] = FixedPoint::from_mul(vec[3] * old[2] + vec[4] * old[6] + vec[5] * old[10]);
        self.elems[7] = FixedPoint::from_mul(vec[3] * old[3] + vec[4] * old[7] + vec[5] * old[11]);

        self.elems[8] = FixedPoint::from_mul(vec[6] * old[0] + vec[7] * old[4] + vec[8] * old[8]);
        self.elems[9] = FixedPoint::from_mul(vec[6] * old[1] + vec[7] * old[5] + vec[8] * old[9]);
        self.elems[10] = FixedPoint::from_mul(vec[6] * old[2] + vec[7] * old[6] + vec[8] * old[10]);
        self.elems[11] = FixedPoint::from_mul(vec[6] * old[3] + vec[7] * old[7] + vec[8] * old[11]);

        self.elems[12] = FixedPoint::from_mul(vec[9] * old[0] + vec[10] * old[4] + vec[11] * old[8] + 0x1000 * old[12]);
        self.elems[13] = FixedPoint::from_mul(vec[9] * old[1] + vec[10] * old[5] + vec[11] * old[9] + 0x1000 * old[13]);
        self.elems[14] = FixedPoint::from_mul(vec[9] * old[2] + vec[10] * old[6] + vec[11] * old[10] + 0x1000 * old[14]);
        self.elems[15] = FixedPoint::from_mul(vec[9] * old[3] + vec[10] * old[7] + vec[11] * old[11] + 0x1000 * old[15]);
    }

    pub fn mul3x3(&mut self, vec: &Vec<u32>) {
        assert_eq!(vec.len(), 9);
        let old = self.elems.clone();
        // self = vec * self
        self.elems[0] = FixedPoint::from_mul(vec[0] * old[0] + vec[1] * old[4] + vec[2] * old[8]);
        self.elems[1] = FixedPoint::from_mul(vec[0] * old[1] + vec[1] * old[5] + vec[2] * old[9]);
        self.elems[2] = FixedPoint::from_mul(vec[0] * old[2] + vec[1] * old[6] + vec[2] * old[10]);
        self.elems[3] = FixedPoint::from_mul(vec[0] * old[3] + vec[1] * old[7] + vec[2] * old[11]);

        self.elems[4] = FixedPoint::from_mul(vec[3] * old[0] + vec[4] * old[4] + vec[5] * old[8]);
        self.elems[5] = FixedPoint::from_mul(vec[3] * old[1] + vec[4] * old[5] + vec[5] * old[9]);
        self.elems[6] = FixedPoint::from_mul(vec[3] * old[2] + vec[4] * old[6] + vec[5] * old[10]);
        self.elems[7] = FixedPoint::from_mul(vec[3] * old[3] + vec[4] * old[7] + vec[5] * old[11]);

        self.elems[8] = FixedPoint::from_mul(vec[6] * old[0] + vec[7] * old[4] + vec[8] * old[8]);
        self.elems[9] = FixedPoint::from_mul(vec[6] * old[1] + vec[7] * old[5] + vec[8] * old[9]);
        self.elems[10] = FixedPoint::from_mul(vec[6] * old[2] + vec[7] * old[6] + vec[8] * old[10]);
        self.elems[11] = FixedPoint::from_mul(vec[6] * old[3] + vec[7] * old[7] + vec[8] * old[11]);
    }

    pub fn scale(&mut self, vec: &Vec<u32>) {
        self.elems[0] = FixedPoint::from_mul(vec[0] * self.elems[0]);
        self.elems[1] = FixedPoint::from_mul(vec[0] * self.elems[1]);
        self.elems[2] = FixedPoint::from_mul(vec[0] * self.elems[2]);
        self.elems[3] = FixedPoint::from_mul(vec[0] * self.elems[3]);

        self.elems[4] = FixedPoint::from_mul(vec[1] * self.elems[4]);
        self.elems[5] = FixedPoint::from_mul(vec[1] * self.elems[5]);
        self.elems[6] = FixedPoint::from_mul(vec[1] * self.elems[6]);
        self.elems[7] = FixedPoint::from_mul(vec[1] * self.elems[7]);

        self.elems[8] = FixedPoint::from_mul(vec[2] * self.elems[8]);
        self.elems[9] = FixedPoint::from_mul(vec[2] * self.elems[9]);
        self.elems[10] = FixedPoint::from_mul(vec[2] * self.elems[10]);
        self.elems[11] = FixedPoint::from_mul(vec[2] * self.elems[11]);
    }

    pub fn translate(&mut self, coords: &Vec<u32>) {
        assert_eq!(coords.len(), 3);
        self.elems[12] += FixedPoint::from_mul(coords[0] * self.elems[0] + coords[1] * self.elems[4] + coords[2] * self.elems[8]);
        self.elems[13] += FixedPoint::from_mul(coords[0] * self.elems[1] + coords[1] * self.elems[5] + coords[2] * self.elems[9]);
        self.elems[14] += FixedPoint::from_mul(coords[0] * self.elems[2] + coords[1] * self.elems[6] + coords[2] * self.elems[10]);
        self.elems[15] += FixedPoint::from_mul(coords[0] * self.elems[3] + coords[1] * self.elems[7] + coords[2] * self.elems[11]);
    }
}

impl Index<usize> for Matrix {
    type Output = FixedPoint;

    fn index(&self, index: usize) -> &Self::Output {
        &self.elems[index]
    }
}

impl Mul for Matrix {
    type Output = Matrix;

    fn mul(self, rhs: Matrix) -> Self::Output {
        let mut rhs = rhs.clone();
        rhs.mul4x4(&self.elems.iter().map(|x| x.0 as u32).collect());
        rhs
    }
}

impl Mul<Vec4> for Matrix {
    type Output = Vec4;

    fn mul(self, rhs: Vec4) -> Self::Output {
        Vec4::new(
            FixedPoint::from_mul(self.elems[0] * rhs[0] + self.elems[4] * rhs[1] + self.elems[8] * rhs[2] + self.elems[12] * rhs[3]),
            FixedPoint::from_mul(self.elems[1] * rhs[0] + self.elems[5] * rhs[1] + self.elems[9] * rhs[2] + self.elems[13] * rhs[3]),
            FixedPoint::from_mul(self.elems[2] * rhs[0] + self.elems[6] * rhs[1] + self.elems[10] * rhs[2] + self.elems[14] * rhs[3]),
            FixedPoint::from_mul(self.elems[3] * rhs[0] + self.elems[7] * rhs[1] + self.elems[11] * rhs[2] + self.elems[15] * rhs[3]),
        )
    }
}

// 12 bit fraction
#[derive(Clone, Copy, PartialEq, PartialOrd)]
pub struct FixedPoint(i32);

impl Mul for FixedPoint {
    type Output = i64;

    fn mul(self, rhs: FixedPoint) -> Self::Output {
        self.0 as i64 * rhs.0 as i64
    }
}

impl Mul<FixedPoint> for u32 {
    type Output = i64;

    // Usually Returns 24 bit fraction
    fn mul(self, rhs: FixedPoint) -> Self::Output {
        self as i32 as i64 * rhs.0 as i64
    }
}

impl FixedPoint {
    pub fn one() -> Self { FixedPoint(1 << 12) }
    pub fn zero() -> Self { FixedPoint(0) }
    pub fn from_mul(val: i64) -> Self { FixedPoint((val >> 12) as i32) }
    pub fn from_num(val: i32) -> Self { FixedPoint(val << 12) }
    pub fn from_frac9(val: u16) -> Self {
        FixedPoint((if (val >> 9) & 0x1 != 0 { 0xFC00 } else { 0x0000 } | val) as i16 as i32)
    }
    pub fn from_frac12(val: i32) -> Self { FixedPoint(val) }
    pub fn from_frac6(val: u16) -> Self { FixedPoint((val as i32) << 6) }
    pub fn num(&self) -> usize { (self.0 >> 12) as usize }
    pub fn raw(&self) -> i32 { self.0 }
}

impl Add<FixedPoint> for i64 {
    type Output = i64;

    fn add(self, rhs: FixedPoint) -> Self::Output {
        self + rhs.0 as i64
    }
}

impl Add for FixedPoint {
    type Output = FixedPoint;

    fn add(self, rhs: Self) -> Self::Output {
        FixedPoint (self.0 + rhs.0)
    }
}

impl AddAssign for FixedPoint {
    fn add_assign(&mut self, rhs: Self) {
        self.0 += rhs.0;
    }

}

impl Neg for FixedPoint {
    type Output = FixedPoint;

    fn neg(self) -> Self::Output {
        FixedPoint(-self.0)
    }
}

impl std::fmt::Debug for FixedPoint {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_fmt(format_args!("0x{:X}", self.0))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Vec4 {
    elems: [FixedPoint; 4]
}

impl Vec4 {
    pub fn new(x: FixedPoint, y: FixedPoint, z: FixedPoint, w: FixedPoint) -> Self {
        Vec4 {
            elems: [x, y, z, w],
        }
    }
}

impl Index<usize> for Vec4 {
    type Output = FixedPoint;

    fn index(&self, index: usize) -> &Self::Output {
        &self.elems[index]
    }
}

