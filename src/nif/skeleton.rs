use std::collections::HashMap;

use bevy::ecs::entity::Entity;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BoneId(pub usize);

#[derive(Debug)]
pub struct BoneData {
    pub id: BoneId,
    pub name: String,
    pub parent: Option<BoneId>,
    pub children: Vec<BoneId>,
    pub entity: Entity,
}

#[derive(Default, Debug)]
pub struct Skeleton {
    pub bones: Vec<BoneData>,            // Arena storing all bone data
    name_to_id: HashMap<String, BoneId>, // Fast lookup of bone ID by name
    roots: Vec<BoneId>,                  // IDs of root bones
}

impl Skeleton {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_bone(&mut self, entity: Entity, name: String, parent_name: Option<&str>) -> BoneId {
        if self.name_to_id.contains_key(&name) {
            panic!("Bone with name '{}' already exists!", name);
        }

        let new_id = BoneId(self.bones.len());
        let parent_id = parent_name.and_then(|p_name| self.name_to_id.get(p_name).copied());

        let bone_data = BoneData {
            id: new_id,
            name: name.clone(),
            parent: parent_id,
            children: Vec::new(),
            entity,
        };
        self.bones.push(bone_data);
        self.name_to_id.insert(name, new_id);

        if let Some(p_id) = parent_id {
            // Need to access bones via id for mutable borrow
            if let Some(parent_node) = self.bones.get_mut(p_id.0) {
                parent_node.children.push(new_id);
            } else {
                // This case should ideally not happen if parent_name resolved to an ID
                eprintln!(
                    "Warning: Parent ID {:?} not found for bone {}",
                    p_id, self.bones[new_id.0].name
                );
            }
        } else {
            self.roots.push(new_id);
        }
        new_id
    }

    pub fn get_bone_by_id(&self, id: BoneId) -> Option<&BoneData> {
        self.bones.get(id.0)
    }

    pub fn get_bone_by_id_mut(&mut self, id: BoneId) -> Option<&mut BoneData> {
        self.bones.get_mut(id.0)
    }

    pub fn get_bone_by_name(&self, name: &str) -> Option<&BoneData> {
        self.name_to_id
            .get(name)
            .and_then(|id| self.get_bone_by_id(*id))
    }

    pub fn get_bone_by_name_mut(&mut self, name: &str) -> Option<&mut BoneData> {
        self.name_to_id
            .get(name)
            .copied()
            .and_then(move |id| self.get_bone_by_id_mut(id))
    }

    pub fn get_parent(&self, bone_id: BoneId) -> Option<&BoneData> {
        self.get_bone_by_id(bone_id)
            .and_then(|b| b.parent)
            .and_then(|p_id| self.get_bone_by_id(p_id))
    }

    pub fn get_children(&self, bone_id: BoneId) -> Option<Vec<&BoneData>> {
        self.get_bone_by_id(bone_id).map(|b| {
            b.children
                .iter()
                .filter_map(|child_id| self.get_bone_by_id(*child_id))
                .collect()
        })
    }
    pub fn get_roots(&self) -> Vec<&BoneData> {
        self.roots
            .iter()
            .filter_map(|root_id| self.get_bone_by_id(*root_id))
            .collect()
    }
    pub fn get_all_children(&self, start_bone_name: &str) -> Vec<&BoneData> {
        let mut children = Vec::new();
        if let Some(start_bone_data) = self.get_bone_by_name(start_bone_name) {
            // Our stack will store tuples of (BoneId, indent_level)
            let mut stack: Vec<(BoneId, usize)> = Vec::new();

            // Push the starting bone with indent level 0
            stack.push((start_bone_data.id, 0));
            children.push(start_bone_data);
            while let Some((current_bone_id, indent_level)) = stack.pop() {
                // The loop continues as long as there are items on the stack

                if let Some(bone_data) = self.get_bone_by_id(current_bone_id) {
                    // Print current bone's name with indentation

                    // Add children to the stack for later processing.
                    // To process children in their natural order (first child first),
                    // we must push them onto the stack in REVERSE order.
                    // The last child pushed will be the first one popped and processed.
                    for &child_id in bone_data.children.iter().rev() {
                        stack.push((child_id, indent_level + 1));
                        children.push(self.get_bone_by_id(child_id).unwrap());
                    }
                } else {
                    eprintln!(
                        "Error: Bone with ID {:?} not found during iterative traversal.",
                        current_bone_id
                    );
                }
            }
        }
        children
    }
}
