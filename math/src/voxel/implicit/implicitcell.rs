use crate::voxel::implicit::Function;
use crate::voxel::Cell;

/// Generate function from implicit function.
pub struct ImplicitCell<F>
where
    F: Function,
{
    /// Lod of this cell
    lod: usize,

    /// Cell resolution
    resolution: (usize, usize, usize),

    /// Tho domain of the function along th x,y,z axis
    domain: ((f32, f32), (f32, f32), (f32, f32)),

    /// The function to evaluate
    function: F,

    /// Invert the inside/outside relation
    invert: bool,

    /// The clamp value for voxel border
    clamp: f32,
}

impl<F> ImplicitCell<F>
where
    F: Function,
{
    pub fn new(function: F) -> ImplicitCell<F> {
        ImplicitCell {
            lod: 0,
            resolution: (32, 32, 32),
            domain: ((-1., 1.), (-1., 1.), (-1., 1.)),
            function,
            clamp: 0.,
            invert: false,
        }
    }

    pub fn with_resolution(self, x: usize, y: usize, z: usize) -> Self {
        assert!(x > 0 && y > 0 && z > 0);
        ImplicitCell {
            resolution: (x, y, z),
            ..self
        }
    }

    pub fn with_lod(self, lod: usize) -> Self {
        ImplicitCell { lod, ..self }
    }

    pub fn with_domain(self, x: (f32, f32), y: (f32, f32), z: (f32, f32)) -> Self {
        assert!(x.0 != x.1);
        assert!(y.0 != y.1);
        assert!(z.0 != z.1);
        ImplicitCell {
            domain: (x, y, z),
            ..self
        }
    }

    pub fn with_clamp(self, clamp: f32) -> Self {
        ImplicitCell { clamp, ..self }
    }

    pub fn with_invert(self) -> Self {
        ImplicitCell { invert: true, ..self }
    }

    pub fn x_domain(&self) -> (f32, f32) {
        self.domain.0
    }

    pub fn y_domain(&self) -> (f32, f32) {
        self.domain.1
    }

    pub fn z_domain(&self) -> (f32, f32) {
        self.domain.2
    }
}

impl<F> Cell for ImplicitCell<F>
where
    F: Function,
{
    fn lod(&self) -> usize {
        self.lod
    }

    fn resolution(&self) -> (usize, usize, usize) {
        self.resolution
    }

    fn get(&self, delta_lod: u32, x: isize, y: isize, z: isize) -> bool {
        if delta_lod != 0 {
            unimplemented!("only delta_lod == 0 is supported");
        }
        let (rx, ry, rz) = (self.resolution.0 as f32, self.resolution.1 as f32, self.resolution.2 as f32);
        let (sx, sy, sz) = ((self.domain.0).0, (self.domain.1).0, (self.domain.2).0);
        let (ex, ey, ez) = ((self.domain.0).1, (self.domain.1).1, (self.domain.2).1);
        let x = (x as f32 + 0.5) as f32 / rx;
        let y = (y as f32 + 0.5) as f32 / ry;
        let z = (z as f32 + 0.5) as f32 / rz;
        let x = x * (ex - sx) + sx;
        let y = y * (ey - sy) + sy;
        let z = z * (ez - sz) + sz;
        self.invert == (self.function.eval(x, y, z) > self.clamp)
    }
}
