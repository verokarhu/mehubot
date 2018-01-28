extern crate rusqlite;

static DB_NAME: &'static str = "/database.sqlite";
static SQL_CREATE_TABLE_MEDIA: &'static str = "CREATE TABLE IF NOT EXISTS media (file_id TEXT NOT NULL, type TEXT NOT NULL);";
static SQL_CREATE_TABLE_TAG: &'static str = "CREATE TABLE IF NOT EXISTS tag (id INT NOT NULL, tag TEXT NOT NULL, counter INT NOT NULL DEFAULT 0, FOREIGN KEY(id) REFERENCES media(rowid), PRIMARY KEY(id, tag));";

static SQL_INSERT_MEDIA: &'static str = "INSERT INTO media (file_id, type) VALUES (?, ?);";
static SQL_INSERT_TAG: &'static str = "INSERT INTO tag (id, tag) VALUES (?, ?);";

static MEDIA_TYPE_PHOTO: &'static str = "photo";

pub struct Connection {
    sqlite_conn: rusqlite::Connection,
}

pub struct DB<'a> {
    statement_cache: StatementCache<'a>,
}

struct StatementCache<'a> {
    insert_media: rusqlite::Statement<'a>,
    insert_tag: rusqlite::Statement<'a>,
}

pub enum Entity {
    Photo { id: i64, file_id: String },
    Tag { id: i64, tag: String, counter: u32 },
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
                            .expect("Failed preparing insert media statement.");

        let insert_tag = c.sqlite_conn
                          .prepare(SQL_INSERT_TAG)
                          .expect("Failed preparing insert tag statement.");

        let statement_cache = StatementCache { insert_media, insert_tag };

        DB { statement_cache }
    }

    pub fn insert(&mut self, entity: Entity) -> i64 {
        match entity {
            Entity::Photo { id, file_id } => {
                info!("Inserting photo with file_id {}", file_id);

                self.statement_cache
                    .insert_media
                    .insert(&[&file_id, &MEDIA_TYPE_PHOTO])
                    .expect("Failed to run insert_media_statement.")
            }
            Entity::Tag { id, tag, counter } => {
                info!("Inserting tag {} to id {}", id, tag);

                self.statement_cache
                    .insert_tag
                    .insert(&[&id, &tag])
                    .expect("Failed to run insert_media_statement.")
            }
        }
    }
}