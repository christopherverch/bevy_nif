use bitflags::bitflags;

pub enum CollisionShapeType {
    Aabb,
    RotatingBox,
    Cylinder,
}

bitflags! {
    /// Port of collisiontype.hpp
    /// These are the layers or groups an object can belong to or collide with.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
    pub struct MWCollisionGroup: u32 {
        const WORLD      = 1 << 0; // CollisionType_World
        const ACTOR      = 1 << 1; // CollisionType_Actor
        const HEIGHTMAP  = 1 << 2; // CollisionType_HeightMap
        const PROJECTILE = 1 << 3; // CollisionType_Projectile
        const DOOR       = 1 << 4; // CollisionType_Door
        const WATER      = 1 << 5; // CollisionType_Water
    }
}
