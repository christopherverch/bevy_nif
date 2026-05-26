use bitflags::bitflags;

pub enum CollisionShapeType {
    Aabb,
    RotatingBox,
    Cylinder,
}

bitflags! {
    /// These are the layers or groups an object can belong to or collide with.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct MWCollisionGroup: u32 {
        const WORLD      = 1 << 0;
        const ACTOR      = 1 << 1;
        const HEIGHTMAP  = 1 << 2;
        const PROJECTILE = 1 << 3;
        const DOOR       = 1 << 4;
        const WATER      = 1 << 5;
    }
}
