extern crate rusqlite;

use self::rusqlite::Error;
use self::rusqlite::types::{Value, ValueRef, ToSql, ToSqlOutput, FromSql, FromSqlResult};

static DB_NAME: &'static str = "/database.sqlite";
static SQL_CREATE_TABLE_MEDIA: &'static str = "CREATE TABLE IF NOT EXISTS media (media_id INTEGER PRIMARY KEY NOT NULL, file_id TEXT UNIQUE NOT NULL, media_type INTEGER NOT NULL);";
static SQL_CREATE_TABLE_TAG: &'static str = "CREATE TABLE IF NOT EXISTS tag (media_id INTEGER NOT NULL, tag TEXT NOT NULL, counter INT NOT NULL DEFAULT 0, FOREIGN KEY(media_id) REFERENCES media(media_id), PRIMARY KEY(media_id, tag));";

static SQL_INSERT_MEDIA: &'static str = "INSERT INTO media (file_id, media_type) VALUES(?, ?);";
static SQL_INSERT_TAG: &'static str = "INSERT INTO tag (media_id, tag) VALUES (?, ?);";

static SQL_READ_TAG: &'static str = "SELECT media_id FROM tag WHERE media_id = ? AND tag = ?;";

static SQL_READ_MEDIA: &'static str = "SELECT DISTINCT a.media_id, file_id, media_type FROM media AS a, tag AS b WHERE a.media_id = b.media_id ORDER BY counter DESC;";
static SQL_READ_MEDIA_WITH_MEDIAID: &'static str = "SELECT media_id, file_id, media_type FROM media AS a WHERE media_id = ?;";
static SQL_READ_MEDIA_WITH_FILEID_AND_TYPE: &'static str = "SELECT media_id FROM media WHERE file_id = ? AND media_type = ?;";
static SQL_READ_MEDIA_WITH_USER_AND_QUERY: &'static str = "SELECT DISTINCT a.media_id, file_id, media_type FROM media AS a, tag AS b WHERE a.media_id = b.media_id AND tag LIKE ? ORDER BY counter DESC;";

static SQL_INCREASE_TAG_COUNTER: &'static str = "UPDATE tag SET counter = counter + 1 WHERE media_id = ? AND tag LIKE ?;";

static SQL_TRANSACTION_BEGIN: &'static str = "BEGIN TRANSACTION;";
static SQL_TRANSACTION_END: &'static str = "END TRANSACTION;";

#[derive(Debug)]
pub enum MediaType {
    Photo,
    Mpeg4Gif,
    ImageGif,
}

impl ToSql for MediaType {
    fn to_sql(&self) -> Result<ToSqlOutput, Error> {
        match self {
            &MediaType::Photo => Ok(ToSqlOutput::Owned(Value::Integer(0))),
            &MediaType::Mpeg4Gif => Ok(ToSqlOutput::Owned(Value::Integer(1))),
            &MediaType::ImageGif => Ok(ToSqlOutput::Owned(Value::Integer(2))),
        }
    }
}

impl FromSql for MediaType {
    fn column_result(value: ValueRef) -> FromSqlResult<Self> {
        match value.as_i64() {
            Ok(0) => Ok(MediaType::Photo),
            Ok(1) => Ok(MediaType::Mpeg4Gif),
            Ok(2) => Ok(MediaType::ImageGif),
            Ok(_) => panic!("Unkown media type"),
            Err(e) => Err(e)
        }
    }
}

pub struct Connection {
    sqlite_conn: rusqlite::Connection,
}

pub struct DB<'a> {
    statement_cache: StatementCache<'a>,
}

struct StatementCache<'a> {
    insert_media: rusqlite::Statement<'a>,
    insert_tag: rusqlite::Statement<'a>,
    read_media: rusqlite::Statement<'a>,
    read_media_with_fileid_and_type: rusqlite::Statement<'a>,
    read_media_with_mediaid: rusqlite::Statement<'a>,
    read_media_with_query: rusqlite::Statement<'a>,
    read_tag: rusqlite::Statement<'a>,
    increase_tag_counter: rusqlite::Statement<'a>,
    transaction_begin: rusqlite::Statement<'a>,
    transaction_end: rusqlite::Statement<'a>,
}

pub enum Entity {
    Media { id: i64, file_id: String, media_type: MediaType },
    Tag { id: i64, media_id: i64, tag: String, counter: i64 },
}

impl Connection {
    pub fn new(path: String) -> Connection {
        Connection {
            sqlite_conn: rusqlite::Connection::open(path + DB_NAME).expect("Failed to open database.")
        }
    }
}

impl<'a> DB<'a> {
    pub fn new(c: &'a Connection) -> DB<'a> {
        c.sqlite_conn
         .execute(SQL_CREATE_TABLE_MEDIA, &[])
         .expect("Unable to create table media.");

        c.sqlite_conn
         .execute(SQL_CREATE_TABLE_TAG, &[])
         .expect("Unable to create table tag.");

        let insert_media = c.sqlite_conn
                            .prepare(SQL_INSERT_MEDIA)
                            .expect("Failed preparing media insert statement.");

        let insert_tag = c.sqlite_conn
                          .prepare(SQL_INSERT_TAG)
                          .expect("Failed preparing tag insert statement.");

        let read_media_with_mediaid = c.sqlite_conn
                                       .prepare(SQL_READ_MEDIA_WITH_MEDIAID)
                                       .expect("Failed preparing media read with mediaid statement.");

        let read_media_with_fileid_and_type = c.sqlite_conn
                                               .prepare(SQL_READ_MEDIA_WITH_FILEID_AND_TYPE)
                                               .expect("Failed preparing media read with fileid and type statement.");

        let read_media = c.sqlite_conn
                          .prepare(SQL_READ_MEDIA)
                          .expect("Failed preparing media read statement.");

        let read_media_with_query = c.sqlite_conn
                                     .prepare(SQL_READ_MEDIA_WITH_USER_AND_QUERY)
                                     .expect("Failed preparing media read with owner and query statement.");

        let read_tag = c.sqlite_conn
                        .prepare(SQL_READ_TAG)
                        .expect("Failed preparing tag read statement.");

        let increase_tag_counter = c.sqlite_conn
                                    .prepare(SQL_INCREASE_TAG_COUNTER)
                                    .expect("Failed preparing increase tag counter statement.");

        let transaction_begin = c.sqlite_conn
                                 .prepare(SQL_TRANSACTION_BEGIN)
                                 .expect("Failed preparing transaction begin statement.");

        let transaction_end = c.sqlite_conn
                               .prepare(SQL_TRANSACTION_END)
                               .expect("Failed preparing transaction begin statement.");

        let statement_cache = StatementCache {
            insert_media,
            insert_tag,
            read_media,
            read_media_with_fileid_and_type,
            read_media_with_mediaid,
            read_media_with_query,
            read_tag,
            increase_tag_counter,
            transaction_begin,
            transaction_end,
        };

        DB { statement_cache }
    }

    pub fn read_media(&mut self) -> Vec<Entity> {
        self.statement_cache
            .read_media
            .query_map(&[],
                       |row| Entity::Media {
                           id: row.get(0),
                           file_id: row.get(1),
                           media_type: row.get(2),
                       })
            .expect("Failed to read media.")
            .filter_map(|r| r.ok())
            .collect()
    }

    pub fn read_media_with_mediaid(&mut self, media_id: i64) -> Entity {
        self.statement_cache
            .read_media_with_mediaid
            .query_map(&[&media_id],
                       |row| Entity::Media {
                           id: row.get(0),
                           file_id: row.get(1),
                           media_type: row.get(2),
                       })
            .expect("Failed to read media with media_id.")
            .filter_map(|r| r.ok())
            .last()
            .unwrap()
    }

    pub fn read_media_with_query(&mut self, query: String) -> Vec<Entity> {
        let query = query + "%";

        self.statement_cache
            .read_media_with_query
            .query_map(&[&query],
                       |row| Entity::Media {
                           id: row.get(0),
                           file_id: row.get(1),
                           media_type: row.get(2),
                       })
            .expect("Failed to read media with query.")
            .filter_map(|r| r.ok())
            .collect()
    }

    pub fn increase_tag_counter(&mut self, media_id: i64, query: String) {
        let query = query + "%";

        self.statement_cache
            .increase_tag_counter
            .execute(&[&media_id, &query])
            .expect("Failed to update tag counter.");
    }

    pub fn insert(&mut self, entity: Entity) -> i64 {
        self.statement_cache
            .transaction_begin
            .execute(&[])
            .expect("Failed to begin transaction.");

        let media_id = match entity {
            Entity::Media { file_id, media_type, .. } => {
                if let Some(row) = self.statement_cache
                                       .read_media_with_fileid_and_type
                                       .query(&[&file_id, &media_type])
                                       .expect("Failed to run read_media statement.")
                                       .next() {
                    row.unwrap().get(0)
                } else {
                    info!("Inserting media with file_id = {} media_type = {:?}", file_id, media_type);

                    self.statement_cache
                        .insert_media
                        .insert(&[&file_id, &media_type])
                        .expect("Failed to run insert_media statement.")
                }
            }
            Entity::Tag { media_id, tag, .. } => {
                let tag = tag.to_lowercase();

                if let Some(row) = self.statement_cache
                                       .read_tag
                                       .query(&[&media_id, &tag])
                                       .expect("Failed to run read_tag statement.")
                                       .next() {
                    row.unwrap().get(0)
                } else {
                    info!("Inserting tag {} to media_id {}", tag, media_id);

                    self.statement_cache
                        .insert_tag
                        .insert(&[&media_id, &tag])
                        .expect("Failed to run insert_tag statement.")
                }
            }
        };

        self.statement_cache
            .transaction_end
            .execute(&[])
            .expect("Failed to end transaction.");

        media_id
    }
}