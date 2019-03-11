use super::{Demon, DemonWithCreatorsAndRecords};
use crate::{
    citext::{CiStr, CiString},
    model::{creator::Creator, Player},
    operation::{Get, Post, PostData},
    permissions::PermissionsSet,
    schema::demons,
    video, Result,
};
use diesel::{insert_into, Connection, PgConnection, RunQueryDsl};
use log::info;
use serde_derive::Deserialize;

#[derive(Deserialize, Debug)]
pub struct PostDemon {
    name: CiString,
    position: i16,
    requirement: i16,
    verifier: CiString,
    publisher: CiString,
    creators: Vec<CiString>,
    video: Option<String>,
}

#[derive(Insertable, Debug)]
#[table_name = "demons"]
pub struct NewDemon<'a> {
    name: &'a CiStr,
    position: i16,
    requirement: i16,
    verifier: i32,
    publisher: i32,
    video: Option<&'a String>,
}

impl Post<PostDemon> for Demon {
    fn create_from(mut data: PostDemon, connection: &PgConnection) -> Result<Demon> {
        info!("Creating new demon from {:?}", data);

        Demon::validate_requirement(&mut data.requirement)?;

        let video = match data.video {
            Some(ref video) => Some(video::validate(video)?),
            None => None,
        };

        connection.transaction(|| {
            Demon::validate_name(&mut data.name, connection)?;
            Demon::validate_position(&mut data.position, connection)?;

            let publisher = Player::get(data.publisher.as_ref(), connection)?;
            let verifier = Player::get(data.verifier.as_ref(), connection)?;

            let new = NewDemon {
                name: data.name.as_ref(),
                position: data.position,
                requirement: data.requirement,
                verifier: verifier.id,
                publisher: publisher.id,
                video: video.as_ref(),
            };

            Demon::shift_down(new.position, connection)?;

            insert_into(demons::table)
                .values(&new)
                .execute(connection)?;

            for creator in &data.creators {
                Creator::create_from((data.name.as_ref(), creator.as_ref()), connection)?;
            }

            Ok(Demon {
                name: data.name,
                position: data.position,
                requirement: data.requirement,
                video: data.video,
                publisher,
                verifier,
            })
        })
    }
}

impl Post<PostDemon> for DemonWithCreatorsAndRecords {
    fn create_from(data: PostDemon, connection: &PgConnection) -> Result<Self> {
        DemonWithCreatorsAndRecords::get(Demon::create_from(data, connection)?, connection)
    }
}

impl PostData for PostDemon {
    fn required_permissions(&self) -> PermissionsSet {
        perms!(ListModerator or ListAdministrator)
    }
}
