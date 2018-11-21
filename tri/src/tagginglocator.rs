use geometry::{CollinearTest, Orientation, Position, Predicates};
use graph::{Face, Vertex};
use indexing::VertexQuery;
use triangulation::Triangulation;
use types::{invalid_face_index, rot3, vertex_index, FaceIndex, FaceVertex, Rot3};

#[derive(Debug)]
enum ContainmentResult {
    Continue(FaceIndex),
    Stop(u8),
}

impl ContainmentResult {
    fn set(&mut self, i: Rot3, b: bool) {
        assert!(i.is_valid());
        if b {
            match *self {
                ContainmentResult::Stop(ref mut t) => *t |= 1 << i.id(),
                _ => unreachable!(),
            }
        }
    }
}

///Result of a point location query
#[derive(Debug)]
pub enum Location {
    /// Point is inside a triangle
    Face(FaceIndex),

    /// Point is on the edge of a triangle
    Edge(FaceIndex, Rot3),

    /// Point is on the vertex of a triangle
    Vertex(FaceIndex, Rot3),

    /// Triangulation is empty
    Empty,

    /// Point is outside the affine hull of a 0D triangulation (the query point and triangulation forms a segment)
    OutsideAffineHull,

    /// Point is outside the affine hull of a 1D triangulation (the query point and triangulation forms a cw triangle)
    OutsideAffineHullClockwise,

    /// Point is outside the affine hull of a 1D triangulation (the query point and triangulation forms a ccw triangle)
    OutsideAffineHullCounterClockwise,

    /// Point is outside the affine hull of a 2D triangulation, with the given nearest face
    OutsideConvexHull(FaceIndex),
}

pub trait TaggingLocator {
    type Position: Position;

    /// Find the exact location a point in the triangulation.
    fn locate_position(&mut self, p: &Self::Position, hint: Option<FaceIndex>) -> Result<Location, String>;
}

impl<PR, V, F> Triangulation<PR, V, F>
where
    PR: Predicates,
    V: Vertex<Position = PR::Position>,
    F: Face,
{
    /// Find the location of a point in a single point triangulation (dimension = 0).
    fn locate_position_dim0(&mut self, p: &PR::Position) -> Result<Location, String> {
        assert!(self.graph.dimension() == 0);

        // find the (only) finite vertex
        let v0 = {
            let v = vertex_index(1);
            if !self.graph.is_infinite_vertex(v) {
                v
            } else {
                vertex_index(0)
            }
        };

        let p0 = &self.graph.pos(v0);
        if self.predicates.test_coincident_points(p, p0) {
            let f0 = self.graph[v0].face();
            Ok(Location::Vertex(f0, self.graph[f0].get_vertex_index(v0).unwrap()))
        } else {
            Ok(Location::OutsideAffineHull)
        }
    }

    /// Find the location of a point in a straight line strip. (dimension = 1)
    fn locate_position_dim1(&mut self, p: &PR::Position) -> Result<Location, String> {
        assert!(self.graph.dimension() == 1);

        // calculate the convex hull of the 1-d mesh
        // the convex hull is a segment made up from the two (finite) neighboring vertices of the infinite vertex

        let vinf = self.graph.infinite_vertex();
        // first point of the convex hull (segments)
        let f0 = self.graph.infinite_face();
        let iv0 = self.graph[f0].get_vertex_index(vinf).unwrap();
        let cp0 = &self.graph.pos(FaceVertex::from(f0, iv0.mirror(2)));

        // last point of the convex hull (segments)
        let f1 = self.graph[f0].neighbor(iv0.mirror(2));
        let iv1 = self.graph[f1].get_vertex_index(vinf).unwrap();
        let cp1 = &self.graph.pos(FaceVertex::from(f1, iv1.mirror(2)));

        let orient = self.predicates.orientation_triangle(cp0, cp1, p);
        if orient.is_cw() {
            Ok(Location::OutsideAffineHullClockwise)
        } else if orient.is_ccw() {
            Ok(Location::OutsideAffineHullCounterClockwise)
        } else {
            // point is on the line
            let t = self.predicates.test_collinear_points(cp0, cp1, p);
            if t.is_before() {
                Ok(Location::OutsideConvexHull(f0))
            } else if t.is_first() {
                Ok(Location::Vertex(f0, iv0.mirror(2)))
            } else if t.is_second() {
                Ok(Location::Vertex(f1, iv1.mirror(2)))
            } else if t.is_after() {
                Ok(Location::OutsideConvexHull(f1))
            } else {
                assert!(t.is_between());
                // Start from an infinite face(f0) and advance to the neighboring segments while the
                // the edge(face) containing the point is not found
                let mut prev = f0;
                let mut dir = iv0;
                loop {
                    let cur = self.graph[prev].neighbor(dir);
                    assert!(self.graph.is_finite_face(cur));

                    let p0 = &self.graph.pos(FaceVertex::from(cur, rot3(0)));
                    let p1 = &self.graph.pos(FaceVertex::from(cur, rot3(1)));

                    let t = self.predicates.test_collinear_points(p0, p1, p);
                    if t.is_first() {
                        // identical to p0
                        return Ok(Location::Vertex(cur, rot3(0)));
                    } else if t.is_second() {
                        // identical to p1
                        return Ok(Location::Vertex(cur, rot3(1)));
                    } else if t.is_between() {
                        // inside the (p0,p1) segment
                        return Ok(Location::Edge(cur, rot3(2)));
                    } else {
                        // advance to the next edge
                        let vi = self.graph[cur].get_neighbor_index(prev).unwrap();
                        prev = cur;
                        dir = vi.mirror(2);
                    }
                }
            }
        }
    }

    /// Test which halfspace contains the p point.
    fn test_containment_face(&self, pos: &PR::Position, face: FaceIndex) -> ContainmentResult {
        let p0 = &self.graph.pos(FaceVertex::from(face, rot3(0)));
        let p1 = &self.graph.pos(FaceVertex::from(face, rot3(1)));
        let p2 = &self.graph.pos(FaceVertex::from(face, rot3(2)));

        let e01 = self.predicates.orientation_triangle(p0, p1, pos);
        if e01.is_cw() {
            let next = self.graph[face].neighbor(rot3(2));
            return ContainmentResult::Continue(next);
        }

        let e20 = self.predicates.orientation_triangle(p2, p0, pos);
        if e20.is_cw() {
            let next = self.graph[face].neighbor(rot3(1));
            return ContainmentResult::Continue(next);
        }

        let e12 = self.predicates.orientation_triangle(&p1, &p2, pos);
        if e12.is_cw() {
            let next = self.graph[face].neighbor(rot3(0));
            return ContainmentResult::Continue(next);
        }

        let mut test = ContainmentResult::Stop(0);
        test.set(rot3(2), e01.is_collinear());
        test.set(rot3(0), e12.is_collinear());
        test.set(rot3(1), e20.is_collinear());
        test
    }

    /// Test the containment of the p position with respect to the half spaces defined by the (a,b) and (b,c) edges.
    fn test_containment(&self, pos: &PR::Position, face: FaceIndex, a: Rot3, b: Rot3, c: Rot3, tag: usize) -> ContainmentResult {
        let pa = &self.graph.pos(FaceVertex::from(face, a));
        let pb = &self.graph.pos(FaceVertex::from(face, b));
        let ab = self.predicates.orientation_triangle(&pa, &pb, pos);

        if ab.is_cw() {
            let next = self.graph[face].neighbor(c);
            if self.graph[next].tag() != tag {
                return ContainmentResult::Continue(next);
            }
        }

        let pc = &self.graph.pos(FaceVertex::from(face, c));
        let bc = self.predicates.orientation_triangle(pb, pc, pos);
        if bc.is_cw() {
            let next = self.graph[face].neighbor(a);
            assert!(self.graph[next].tag() != tag);
            return ContainmentResult::Continue(next);
        }

        let mut test = ContainmentResult::Stop(0);
        test.set(c, ab.is_collinear());
        test.set(a, bc.is_collinear());
        test
    }

    // Find the location of a point in a triangulation. (dimension = 2)
    fn locate_position_dim2(&mut self, p: &PR::Position, hint: Option<FaceIndex>) -> Result<Location, String> {
        assert_eq!(self.graph.dimension(), 2);

        let start = {
            let hint = hint.unwrap_or_else(|| self.graph.infinite_face());
            match self.graph[hint].get_vertex_index(self.graph.infinite_vertex()) {
                None => hint,                            // finite face
                Some(i) => self.graph[hint].neighbor(i), // the opposite face to an infinite vertex is finite
            }
        };
        assert!(self.graph.is_finite_face(start));

        let mut prev = invalid_face_index();
        let mut cur = start;
        //let mut count = 0;

        // keep a mutable reference to tag to avoid any additional interference in tag increment during traverse
        let mut tag = self.tag.borrow_mut();
        *tag += 1;

        loop {
            if self.graph.is_infinite_face(cur) {
                return Ok(Location::OutsideConvexHull(cur));
            }

            self.graph[cur].set_tag(*tag);
            let from = self.graph[cur].get_neighbor_index(prev);

            let test_result = match from.map(|r| r.id()) {
                None => self.test_containment_face(p, cur),
                Some(0) => self.test_containment(p, cur, rot3(2), rot3(0), rot3(1), *tag),
                Some(1) => self.test_containment(p, cur, rot3(0), rot3(1), rot3(2), *tag),
                Some(2) => self.test_containment(p, cur, rot3(1), rot3(2), rot3(0), *tag),
                Some(i) => unreachable!(format!("Invalid index: {:?}", i)),
            };
            match test_result {
                ContainmentResult::Continue(next) => {
                    prev = cur;
                    cur = next;
                    //count += 1;
                } // continue, already updated

                ContainmentResult::Stop(0) => return Ok(Location::Face(cur)),
                ContainmentResult::Stop(1) => return Ok(Location::Edge(cur, rot3(0))), // only on 0 edge
                ContainmentResult::Stop(2) => return Ok(Location::Edge(cur, rot3(1))), // only on 1 edge
                ContainmentResult::Stop(4) => return Ok(Location::Edge(cur, rot3(2))), // only on 2 edge
                ContainmentResult::Stop(6) => return Ok(Location::Vertex(cur, rot3(0))), //both on 1,2 edge
                ContainmentResult::Stop(5) => return Ok(Location::Vertex(cur, rot3(1))), //both on 0,2 edge
                ContainmentResult::Stop(3) => return Ok(Location::Vertex(cur, rot3(2))), //both on 0,1 edge

                ContainmentResult::Stop(e) => unreachable!("Invalid test_result: {}", e),
            }
        }
    }
}

impl<PR, V, F> TaggingLocator for Triangulation<PR, V, F>
where
    PR: Predicates,
    V: Vertex<Position = PR::Position>,
    F: Face,
{
    type Position = PR::Position;

    fn locate_position(&mut self, p: &PR::Position, hint: Option<FaceIndex>) -> Result<Location, String> {
        match self.graph.dimension() {
            -1 => Ok(Location::Empty),
            0 => self.locate_position_dim0(p),
            1 => self.locate_position_dim1(p),
            2 => self.locate_position_dim2(p, hint),
            dim => unreachable!(format!("Invalid dimension: {}", dim)),
        }
    }
}
