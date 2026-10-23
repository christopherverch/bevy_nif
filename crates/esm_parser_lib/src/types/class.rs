use crate::{
    EsmError, EsmReader, Load, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags, skill::SkillIndex},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Class {
    pub record_flags: ObjectFlags,
    pub name: String,
    pub description: String,
    pub id: RefId,
    pub data: CLDTstruct,
}

impl EsmRecord for Class {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Clas;
}

impl Class {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut class = Class::default();
        class.record_flags = reader.get_record_flags();

        let mut has_name = false;
        let mut has_data = false;
        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    class.id = reader.read_refid()?;
                    has_name = true;
                }
                b"FNAM" => {
                    class.name = reader.read_header_decode_string()?;
                }
                b"CLDT" => {
                    reader.read_sub_header()?;
                    class.data = reader.load()?;
                    if class.data.is_playable > 1 {
                        return Err(reader.fail("Unknown bool value"));
                    }
                    has_data = true;
                }
                b"DESC" => {
                    class.description = reader.read_header_decode_string()?;
                }
                b"DELE" => {
                    reader.skip_sub_header()?;
                    *is_deleted = true;
                }
                _ => {
                    return Err(reader.fail("Unknown subrecord"));
                }
            }
        }

        if !has_name {
            return Err(reader.fail("Missing NAME subrecord"));
        }
        if !has_data && !*is_deleted {
            return Err(reader.fail("Missing CLDT subrecord"));
        }

        Ok(class)
    }
}
impl Class {
    pub fn blank(&mut self) {
        self.record_flags = ObjectFlags::default();
        self.name.clear();
        self.description.clear();
        self.data = CLDTstruct::default();
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CLDTstruct {
    pub attribute: [i32; 2],
    pub specialization: i32,
    pub skills: [[SkillIndex; 2]; 5],
    pub is_playable: i32,
    pub services: i32,
}

impl Load for CLDTstruct {
    fn load(reader: &mut EsmReader) -> Result<CLDTstruct, EsmError> {
        let [
            attribute0,
            attribute1,
            specialization,
            skills00,
            skills01,
            skills10,
            skills11,
            skills20,
            skills21,
            skills30,
            skills31,
            skills40,
            skills41,
            is_playable,
            services,
        ] = reader.load()?;

        let parse_skill = |val: i32| -> Result<SkillIndex, EsmError> {
            SkillIndex::try_from(val)
                .map_err(|_| EsmError::Custom(format!("Invalid skill index: {}", val)))
        };
        Ok(Self {
            attribute: [attribute0, attribute1],
            specialization,
            skills: [
                [parse_skill(skills00)?, parse_skill(skills01)?],
                [parse_skill(skills10)?, parse_skill(skills11)?],
                [parse_skill(skills20)?, parse_skill(skills21)?],
                [parse_skill(skills30)?, parse_skill(skills31)?],
                [parse_skill(skills40)?, parse_skill(skills41)?],
            ],
            is_playable,
            services,
        })
    }
}

impl CLDTstruct {
    pub fn get_skill(&self, index: usize, major: bool) -> Result<SkillIndex, EsmError> {
        if index >= self.skills.len() {
            return Err(EsmError::Custom(format!(
                "Invalid skill row index: {}",
                index
            )));
        }
        Ok(self.skills[index][major as usize])
    }
}
