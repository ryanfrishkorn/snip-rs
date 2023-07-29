use chrono::{DateTime, FixedOffset};
use rusqlite::{Connection, DatabaseName};
use std::error::Error;
use std::io::Read;
use std::path::Path;
use uuid::Uuid;

use crate::snip::SnipError;

/// Attachment represents binary data attached to a document
pub struct Attachment {
    pub uuid: Uuid,
    pub snip_uuid: Uuid,
    pub timestamp: DateTime<FixedOffset>,
    pub name: String,
    pub data: Vec<u8>,
    pub size: usize,
}

impl Attachment {
    /// Remove attachment by Uuid
    pub fn remove(&self, conn: &Connection) -> Result<(), Box<dyn Error>> {
        let mut stmt = conn.prepare("DELETE FROM snip_attachment WHERE uuid = :uuid")?;
        let rows_affected = stmt.execute(&[(":uuid", &self.uuid.to_string())])?;
        if rows_affected != 1 {
            return Err(Box::new(SnipError::General(format!("expected 1 row affected, got {}", rows_affected))));
        }
        Ok(())
    }
}

/// Returns an Attachment struct parsed from the database
fn attachment_data_from_db(conn: &Connection, row_id: i64) -> Result<Vec<u8>, Box<dyn Error>> {
    let mut blob = conn.blob_open(DatabaseName::Main, "snip_attachment", "data", row_id, true)?;
    let mut data: Vec<u8> = Vec::new();

    let _bytes_read = blob.read_to_end(&mut data)?;
    Ok(data)
}

/// Returns an Attachment struct parsed from the database
fn attachment_from_db(
    uuid: String,
    snip_uuid: String,
    timestamp: String,
    name: String,
    size: usize,
    data: Vec<u8>,
) -> Result<Attachment, Box<dyn Error>> {
    let uuid = Uuid::try_parse(uuid.as_str())?;
    let snip_uuid = Uuid::try_parse(snip_uuid.as_str())?;
    let timestamp = DateTime::parse_from_rfc3339(timestamp.as_str())?;

    Ok(Attachment {
        uuid,
        snip_uuid,
        timestamp,
        name,
        size,
        data,
    })
}

/// Add an attachment to the database and attach to supplied document Uuid
pub fn add_attachment(conn: &Connection, snip_uuid: Uuid, path: &Path) -> Result<(), Box<dyn Error>> {
    // check existence of file
    let uuid = Uuid::new_v4();
    let timestamp_utc = chrono::Utc::now();
    let timestamp = timestamp_utc.fixed_offset();
    let name = path.file_name().ok_or("parsing attachment basename")?.to_string_lossy().to_string();
    let data = std::fs::read(path)?;
    let size = data.len();

    // assign new Attachment
    let a = Attachment {
        uuid,
        snip_uuid,
        timestamp,
        name,
        data,
        size,
    };

    // insert
    let mut stmt = conn.prepare("INSERT INTO snip_attachment(uuid, snip_uuid, timestamp, name, data, size) VALUES(:uuid, :snip_uuid, :timestamp, :name, ZEROBLOB(:size), :size)")?;
    let result = stmt.execute(&[
        (":uuid", &a.uuid.to_string()),
        (":snip_uuid", &a.snip_uuid.to_string()),
        (":timestamp", &a.timestamp.to_rfc3339().to_string()),
        (":name", &a.name.to_string()),
        (":size", &a.size.to_string()),
    ])?;
    assert_eq!(result, 1);

    // add blob data
    let row_id = conn.last_insert_rowid();
    let mut blob = conn.blob_open(DatabaseName::Main, "snip_attachment", "data", row_id, false)?;
    blob.write_at(a.data.as_slice(), 0)?;
    Ok(())
}

/// Get an attachment from database
pub fn get_attachment_from_uuid(conn: &Connection, id: Uuid) -> Result<Attachment, Box<dyn Error>> {
    // get metadata
    let mut stmt = conn
        .prepare("SELECT uuid, snip_uuid, timestamp, name, size, rowid FROM snip_attachment WHERE uuid = :id")?;
    let mut rows = stmt.query_and_then(&[(":id", &id.to_string())], |row| {
        // read data first using rowid
        let row_id: i64 = row.get(5)?;
        let data = attachment_data_from_db(conn, row_id)?;
        attachment_from_db(row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?, data)
    })?;

    if let Some(a) = rows.next() {
        let attachment = match a {
            Ok(v) => v,
            Err(e) => return Err(e),
        };
        return Ok(attachment);
    }

    // no rows were returned at this point
    Err(Box::new(SnipError::UuidNotFound(
        "could not find uuid".to_string(),
    )))
}

/// Return a vector of all attachment uuids
pub fn get_attachment_all(conn: &Connection) -> Result<Vec<Uuid>, Box<dyn Error>> {
    let mut stmt = conn.prepare("SELECT uuid FROM snip_attachment")?;
    let query_iter = stmt.query_and_then([], |row| row.get::<_, String>(0))?;

    let mut ids: Vec<Uuid> = Vec::new();
    for id in query_iter {
        let id_str = id.unwrap();
        let id_parsed = Uuid::try_parse(id_str.as_str())?;
        ids.push(id_parsed);
    }
    Ok(ids)
}

/// Search for a attachment uuid matching the supplied partial string.
pub fn search_attachment_uuid(conn: &Connection, id_partial: &str) -> Result<Uuid, Box<dyn Error>> {
    let mut stmt = conn.prepare("SELECT uuid from snip_attachment WHERE uuid LIKE :id LIMIT 2")?;
    let id_partial_fuzzy = format!("{}{}{}", "%", id_partial, "%");

    let rows = stmt.query_map(&[(":id", &id_partial_fuzzy)], |row| {
        let id_str: String = row.get(0)?;
        Ok(id_str)
    })?;

    // return only if a singular result is matched, so we check for two results
    let mut id_str = String::new();
    for (i, id) in rows.into_iter().enumerate() {
        if i == 0 {
            id_str = id.unwrap();
        } else {
            return Err(Box::new(SnipError::UuidMultipleMatches(format!("provided partial {} returned multiple attachment uuids", id_partial))));
        }
    }

    if !id_str.is_empty() {
        return match Uuid::parse_str(&id_str) {
            Ok(v) => Ok(v),
            Err(e) => Err(Box::new(e)),
        };
    }
    Err(Box::new(SnipError::UuidNotFound(format!("attachment uuid not found using partial {}", id_partial))))
}

#[cfg(test)]
mod test {
    use super::*;
    use std::collections::HashMap;
    use crate::snip::test_prep::*;
    use crate::snip::SnipError;

    #[test]
    fn test_add_attachment() -> Result<(), Box<dyn Error>> {
        let conn = prepare_database().expect("preparing in-memory database");

        let snip_uuid = Uuid::try_parse(ID_STR)?;
        let path_str = "test_data/attachments/udhr.pdf";
        let path = Path::new(path_str);
        add_attachment(&conn, snip_uuid, path)?;

        // print out attachments to verify
        let attachments = get_attachment_all(&conn)?;
        for id in attachments {
            let a = get_attachment_from_uuid(&conn, id)?;
            println!("uuid: {} snip_uud: {} size: {} name: {}", a.uuid, a.snip_uuid, a.size, a.name);
        }
        Ok(())
    }

    #[test]
    fn test_get_attachment_from_uuid() -> Result<(), Box<dyn Error>> {
        let conn = prepare_database().expect("preparing in-memory database");

        let id = Uuid::try_parse(ID_ATTACH_STR).expect("parsing attachment uuid string");
        let a = get_attachment_from_uuid(&conn, id)?;

        if a.uuid != id {
            return Err(Box::new(SnipError::UuidNotFound(format!("uuid expected: {} got: {}", id, a.uuid).to_string())));
        }
        Ok(())
    }

    #[test]
    fn test_remove() -> Result<(), Box<dyn Error>> {
        let conn = prepare_database().expect("preparing in-memory database");

        let id = Uuid::try_parse(ID_ATTACH_STR)?;
        let a = get_attachment_from_uuid(&conn, id)?;
        a.remove(&conn)?;

        // attempt to retrieve again - should be missing
        if get_attachment_from_uuid(&conn, id).is_ok() {
            return Err(Box::new(SnipError::General("found attachment in database after attempted removal".to_string())));
        }
        Ok(())
    }

    #[test]
    fn test_search_attachment_uuid() -> Result<(), Box<dyn Error>> {
        let conn = prepare_database().expect("preparing in-memory database");

        let partials: HashMap<String, String> = HashMap::from([           // 9cfc5a2d-2946-48ee-82e0-227ba4bcdbd5
            (ID_ATTACH_STR[0..8].to_string(), "segment 1".to_string()),   // 9cfc5a2d
            (ID_ATTACH_STR[9..13].to_string(), "segment 2".to_string()),  // _________2946
            (ID_ATTACH_STR[14..18].to_string(), "segment 3".to_string()), // ______________48ee
            (ID_ATTACH_STR[19..23].to_string(), "segment 4".to_string()), // ___________________82e0
            (ID_ATTACH_STR[24..].to_string(), "segment 5".to_string()),   // ________________________227ba4bcdbd5
            (ID_ATTACH_STR[7..12].to_string(), "partial 1".to_string()),  // _______d-294
            (ID_ATTACH_STR[7..14].to_string(), "partial 2".to_string()),  // _______d-2946-
            (ID_ATTACH_STR[7..15].to_string(), "partial 3".to_string()),  // _______d-2946-4
            (ID_ATTACH_STR[8..19].to_string(), "partial 4".to_string()),  // ________-2946-48ee-
            (ID_ATTACH_STR[23..].to_string(), "partial 5".to_string()),   // _______________________-227ba4bcdbd5
        ]);

        /*
        println!("9cfc5a2d-2946-48ee-82e0-227ba4bcdbd5");
        for p in &partials {
            println!("{} {}", p.0, p.1);
        }
         */

        let expect = match Uuid::parse_str(ID_ATTACH_STR) {
            Ok(v) => v,
            Err(e) => panic!("{}", e),
        };
        println!("expecting: {expect}");

        // test all uuid string partials
        for p in &partials {
            println!("attachment partial uuid string: {}", p.0);
            let id = search_attachment_uuid(&conn, p.0);
            match id {
                Ok(v) => assert_eq!(expect, v),
                Err(e) => panic!("{}, full: {}, partial: {}", e, ID_ATTACH_STR, &p.0),
            }
        }
        Ok(())
    }
}
