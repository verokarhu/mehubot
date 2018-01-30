extern crate rusqlite;

static DB_NAME: &'static str = "/database.sqlite";
static SQL_CREATE_TABLE_MEDIA: &'static str = "CREATE TABLE IF NOT EXISTS media (file_id TEXT NOT NULL, media_type INTEGER NOT NULL, PRIMARY KEY(file_id, media_type));";
static SQL_CREATE_TABLE_TAG: &'static str = "CREATE TABLE IF NOT EXISTS tag (media_id INTEGER NOT NULL, tag TEXT NOT NULL, counter INT NOT NULL DEFAULT 0, FOREIGN KEY(media_id) REFERENCES media(rowid), PRIMARY KEY(media_id, tag));";
static SQL_CREATE_TABLE_ACCESS: &'static str = "CREATE TABLE IF NOT EXISTS access (media_id INTEGER NOT NULL, owner_id INT NOT NULL DEFAULT 0, owner_type INTEGER NOT NULL, FOREIGN KEY(media_id) REFERENCES media(rowid), PRIMARY KEY(media_id, owner_id));";

static SQL_INSERT_MEDIA: &'static str = "INSERT INTO media (file_id, media_type) VALUES (?, ?);";
static SQL_INSERT_TAG: &'static str = "INSERT INTO tag (media_id, tag) VALUES (?, ?);";
static SQL_INSERT_ACCESS: &'static str = "INSERT INTO access (media_id, owner_id, owner_type) VALUES (?, ?, ?);";

pub enum MediaType {
    Photo,
}

pub enum OwnerType {
    User,
    Group,
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
    insert_access: rusqlite::Statement<'a>,
}

pub enum Entity {
    Media { id: i64, file_id: String, media_type: MediaType },
    Tag { id: i64, media_id: i64, tag: String, counter: i64 },
    Access { id: i64, media_id: i64, owner_id: i64, owner_type: OwnerType },
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

        c.sqlite_conn
         .execute(SQL_CREATE_TABLE_ACCESS, &[])
         .expect("Unable to create table access.");

        let insert_media = c.sqlite_conn
                            .prepare(SQL_INSERT_MEDIA)
                            .expect("Failed preparing media insert statement.");

        let insert_tag = c.sqlite_conn
                          .prepare(SQL_INSERT_TAG)
                          .expect("Failed preparing tag insert statement.");

        let insert_access = c.sqlite_conn
                             .prepare(SQL_INSERT_ACCESS)
                             .expect("Failed preparing access insert statement.");

        let statement_cache = StatementCache { insert_media, insert_tag, insert_access };

        DB { statement_cache }
    }

    pub fn insert(&mut self, entity: Entity) -> i64 {
        match entity {
            Entity::Media { id, file_id, media_type } => {
                let media_type = media_type as u8;

                info!("Inserting media with file_id = {} media_type = {}", file_id, media_type);

                self.statement_cache
                    .insert_media
                    .insert(&[&file_id, &media_type])
                    .expect("Failed to run insert_media_statement.")
            }
            Entity::Tag { id, media_id, tag, counter } => {
                info!("Inserting tag {} to media_id {}", media_id, tag);

                self.statement_cache
                    .insert_tag
                    .insert(&[&media_id, &tag])
                    .expect("Failed to run insert_media_statement.")
            }
            Entity::Access { id, media_id, owner_id, owner_type } => {
                let owner_type = owner_type as u8;

                info!("Inserting access to owner_id {} of owner_type {} to media_id {}", owner_id, owner_type, media_id);

                self.statement_cache
                    .insert_access
                    .insert(&[&media_id, &owner_id, &owner_type])
                    .expect("Failed to run insert_media_statement.")
            }
        }
    }
}