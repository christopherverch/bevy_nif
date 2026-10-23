use crate::{
    EsmError, EsmReader, Load, RefId,
    esm_record::EsmRecord,
    types::{enums::RecInt, flags::ObjectFlags},
};

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RankData {
    pub attribute1: i32,
    pub attribute2: i32,
    pub primary_skill: i32,
    pub favoured_skill: i32,
    pub fact_reputation: i32,
}

impl Load for RankData {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        let attribute1 = reader.read_typed::<i32>(ConsumptionType::SubrecordPayload)?;
        let attribute2 = reader.read_typed::<i32>(ConsumptionType::SubrecordPayload)?;
        let primary_skill = reader.read_typed::<i32>(ConsumptionType::SubrecordPayload)?;
        let favoured_skill = reader.read_typed::<i32>(ConsumptionType::SubrecordPayload)?;
        let fact_reputation = reader.read_typed::<i32>(ConsumptionType::SubrecordPayload)?;

        Ok(Self {
            attribute1,
            attribute2,
            primary_skill,
            favoured_skill,
            fact_reputation,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct FADTstruct {
    pub attribute: [i32; 2],
    pub rank_data: [RankData; 10],
    pub skills: [i32; 7],
    pub is_hidden: i32,
}

impl FADTstruct {
    pub fn get_skill(&self, index: usize) -> Result<i32, EsmError> {
        if index >= self.skills.len() {
            return Err(EsmError::InvalidIndex(format!("Skill index {} out of range", index)));
        }
        Ok(self.skills[index])
    }
}

impl Load for FADTstruct {
    fn load(reader: &mut EsmReader) -> Result<Self, EsmError> {
        reader.read_sub_header()?;
        let attribute = [
            reader.read_typed::<i32>(ConsumptionType::SubrecordPayload)?,
            reader.read_typed::<i32>(ConsumptionType::SubrecordPayload)?,
        ];
        let mut rank_data = [RankData::default(); 10];
        for rank in rank_data.iter_mut() {
            *rank = reader.load()?;
        }
        let mut skills = [0; 7];
        for skill in skills.iter_mut() {
            *skill = reader.read_typed::<i32>(ConsumptionType::SubrecordPayload)?;
        }
        let is_hidden = reader.read_typed::<i32>(ConsumptionType::SubrecordPayload)?;

        Ok(Self {
            attribute,
            rank_data,
            skills,
            is_hidden,
        })
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Faction {
    pub record_flags: u32,
    pub name: String,
    pub id: RefId,
    pub data: FADTstruct,
    pub reactions: std::collections::HashMap<RefId, i32>,
    pub ranks: [String; 10],
}

impl EsmRecord for Faction {
    fn id(&self) -> &RefId {
        &self.id
    }
    const RECORD_ID: RecInt = RecInt::Fact;
}

impl Faction {
    pub fn load(reader: &mut EsmReader, is_deleted: &mut bool) -> Result<Self, EsmError> {
        *is_deleted = false;
        let mut faction = Faction::default();
        faction.record_flags = reader.get_record_flags();
        while reader.has_more_subs() {
            let sub_name = reader.get_or_read_sub_name()?;

            match sub_name.as_bytes() {
                b"NAME" => {
                    faction.id = reader.read_refid()?;
                }
                b"FNAM" => {
                    faction.name = reader.read_header_decode_string()?;
                }
                b"RNAM" => {
                    let index = reader.context.index as usize;
                    if index < faction.ranks.len() {
                        faction.ranks[index] = reader.read_header_decode_string()?;
                    } else {
                        return Err(reader.fail("Rank index out of range"));
                    }
                }
                b"FADT" => {
                    faction.data = reader.load()?;
                }
                b"ANAM" => {
                    let faction_id = reader.read_refid()?;
                    let reaction = reader.read_typed::<i32>(ConsumptionType::SubrecordPayload)?;
                    faction.reactions.insert(faction_id, reaction);
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
        Ok(faction)
    }

    pub fn blank(&mut self) {
        self.record_flags = 0;
        self.name.clear();
        self.data.attribute = [0; 2];
        self.data.rank_data = [RankData::default(); 10];
        self.data.skills = [0; 7];
        self.data.is_hidden = 0;
        self.reactions.clear();
        self.ranks = [String::new(); 10];
    }
}
