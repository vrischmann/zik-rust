extern crate directories;
extern crate rusqlite;
use std::result::Result;

#[derive(Debug)]
enum OpenDatabaseError {
    SQLiteError(rusqlite::Error),
    UnknownDataFolder,
}

impl From<rusqlite::Error> for OpenDatabaseError {
    fn from(err: rusqlite::Error) -> OpenDatabaseError {
        OpenDatabaseError::SQLiteError(err)
    }
}

fn open_database() -> Result<rusqlite::Connection, OpenDatabaseError> {
    if let Some(project_directories) = directories::ProjectDirs::from("fr", "rischmann", "zik") {
        println!("data dir: {:?}", project_directories.data_dir());

        let db_path = project_directories.data_dir().join("data.db");
        let connection = rusqlite::Connection::open(db_path)?;

        Ok(connection)
    } else {
        Err(OpenDatabaseError::UnknownDataFolder)
    }
}

#[derive(Debug)]
enum InitDatabaseError {
    SQLiteError(rusqlite::Error),
}
impl From<rusqlite::Error> for InitDatabaseError {
    fn from(err: rusqlite::Error) -> InitDatabaseError {
        InitDatabaseError::SQLiteError(err)
    }
}

fn init_database(db: &mut rusqlite::Connection) -> Result<(), InitDatabaseError> {
    let ddls = vec![
        "CREATE TABLE IF NOT EXISTS config(
          key TEXT UNIQUE,
          value ANY
        )",
        "CREATE TABLE IF NOT EXISTS artist(
          id INTEGER PRIMARY KEY,
          name TEXT
        ) STRICT",
        "CREATE INDEX IF NOT EXISTS artist_name ON artist(name)",
        "CREATE TABLE IF NOT EXISTS album(
          id INTEGER PRIMARY KEY,
          name TEXT,
          artist_id INTEGER,
          album_artist_id INTEGER,
          release_date TEXT,

          FOREIGN KEY(artist_id) REFERENCES artist(id)
        ) STRICT",
        "CREATE INDEX IF NOT EXISTS album_name ON album(name)",
        "CREATE TABLE IF NOT EXISTS track(
          id INTEGER PRIMARY KEY,
          name TEXT UNIQUE,
          artist_id INTEGER,
          album_id INTEGER,
          release_date TEXT,
          number INTEGER,

          FOREIGN KEY(artist_id) REFERENCES artist(id),
          FOREIGN KEY(album_id) REFERENCES album(id)
        ) STRICT",
    ];

    let savepoint = db.savepoint()?;

    for ddl in ddls {
        match savepoint.execute(ddl, []) {
            Ok(_) => {}
            Err(err) => println!("unable to execute statement, err: {}", err),
        }
    }

    savepoint.commit()?;

    Ok(())
}

#[derive(Debug)]
enum AppError {
    OpenDatabaseError(OpenDatabaseError),
    InitDatabaseError(InitDatabaseError),
}

impl From<OpenDatabaseError> for AppError {
    fn from(err: OpenDatabaseError) -> AppError {
        AppError::OpenDatabaseError(err)
    }
}
impl From<InitDatabaseError> for AppError {
    fn from(err: InitDatabaseError) -> AppError {
        AppError::InitDatabaseError(err)
    }
}

fn main() -> Result<(), AppError> {
    let mut database = open_database()?;
    init_database(&mut database)?;

    println!("database: {:?}", database);

    Ok(())
}
