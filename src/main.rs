extern crate clap;
extern crate directories;
extern crate rusqlite;

use clap::{Arg, Command};
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};
use std::result::Result;

#[derive(Debug)]
enum OpenDatabaseError {
    SQLite(rusqlite::Error),
    DataFolderNotFound,
}
impl fmt::Display for OpenDatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            OpenDatabaseError::SQLite(err) => {
                write!(f, "SQLite error while opening database: {}", err)
            }
            OpenDatabaseError::DataFolderNotFound => write!(f, "data folder for Zik not found"),
        }
    }
}
impl From<rusqlite::Error> for OpenDatabaseError {
    fn from(err: rusqlite::Error) -> OpenDatabaseError {
        OpenDatabaseError::SQLite(err)
    }
}

fn open_database() -> Result<rusqlite::Connection, OpenDatabaseError> {
    if let Some(project_directories) = directories::ProjectDirs::from("fr", "rischmann", "zik") {
        let db_path = project_directories.data_dir().join("data.db");
        let connection = rusqlite::Connection::open(db_path)?;

        Ok(connection)
    } else {
        Err(OpenDatabaseError::DataFolderNotFound)
    }
}

#[derive(Debug)]
enum InitDatabaseError {
    SQLite(rusqlite::Error),
}
impl From<rusqlite::Error> for InitDatabaseError {
    fn from(err: rusqlite::Error) -> InitDatabaseError {
        InitDatabaseError::SQLite(err)
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
enum Config {
    Library(PathBuf),
    ScanParallelism(usize),
}
impl fmt::Display for Config {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Config::Library(val) => write!(f, "{}", val.display()),
            Config::ScanParallelism(val) => write!(f, "{}", val),
        }
    }
}
impl rusqlite::ToSql for Config {
    fn to_sql(&self) -> Result<rusqlite::types::ToSqlOutput<'_>, rusqlite::Error> {
        match self {
            Config::Library(path) => {
                let path_data = path.to_string_lossy().to_string();
                Ok(rusqlite::types::ToSqlOutput::from(path_data))
            }
            Config::ScanParallelism(n) => {
                let new_n = *n as i64;
                Ok(rusqlite::types::ToSqlOutput::from(new_n))
            }
        }
    }
}
impl Config {
    const VALID_KEYS: [&'static str; 2] = ["library", "scan_parallelism"];

    fn is_valid_key(key: &str) -> bool {
        return Config::VALID_KEYS.contains(&key);
    }
}

enum CommandConfigError {
    SQLite(rusqlite::Error),
    InvalidKey(String),
    NoValue(String),
    GetLibraryPath(GetLibraryPathError),
    InvalidScanParallelismValue(std::num::ParseIntError),
}
impl From<rusqlite::Error> for CommandConfigError {
    fn from(err: rusqlite::Error) -> CommandConfigError {
        CommandConfigError::SQLite(err)
    }
}
impl From<GetLibraryPathError> for CommandConfigError {
    fn from(err: GetLibraryPathError) -> CommandConfigError {
        CommandConfigError::GetLibraryPath(err)
    }
}
impl fmt::Display for CommandConfigError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            CommandConfigError::SQLite(err) => write!(f, "SQLite error, {}", err),
            CommandConfigError::InvalidKey(key) => write!(f, "key name `{}` is invalid", key),
            CommandConfigError::NoValue(key) => write!(f, "no value for key name `{}`", key),
            CommandConfigError::GetLibraryPath(err) => {
                write!(f, "could not resolve library path: {}", err)
            }
            CommandConfigError::InvalidScanParallelismValue(err) => {
                write!(f, "`scan_parallelism` value \"{}\" is invalid", err)
            }
        }
    }
}

enum GetLibraryPathError {
    DoestNotExist(PathBuf),
    NotADirectory(PathBuf),
    IO(io::Error),
}
impl From<io::Error> for GetLibraryPathError {
    fn from(err: io::Error) -> GetLibraryPathError {
        GetLibraryPathError::IO(err)
    }
}
impl fmt::Display for GetLibraryPathError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            GetLibraryPathError::DoestNotExist(path) => {
                write!(f, "path \"{}\" does not exist", path.display())
            }
            GetLibraryPathError::NotADirectory(path) => {
                write!(f, "path \"{}\" is not a directory", path.display())
            }
            GetLibraryPathError::IO(err) => write!(f, "unable to access path, err: {}", err),
        }
    }
}

fn get_library_path(value: &str) -> Result<PathBuf, GetLibraryPathError> {
    let path = Path::new(value);
    if !path.exists() {
        return Err(GetLibraryPathError::DoestNotExist(path.to_path_buf()));
    }
    if !path.is_dir() {
        return Err(GetLibraryPathError::NotADirectory(path.to_path_buf()));
    }
    path.metadata()?;

    let canonicalized_path = std::fs::canonicalize(path)?;

    Ok(canonicalized_path.to_path_buf())
}

fn cmd_config(
    db: &mut rusqlite::Connection,
    args: &clap::ArgMatches,
) -> Result<(), CommandConfigError> {
    if args.args_present() {
        if args.is_present("key") && args.is_present("value") {
            let key = args.value_of("key").unwrap();
            let value = args.value_of("value").unwrap();

            let config: Config = match key {
                "library" => {
                    let dir = get_library_path(value)?;
                    Config::Library(dir)
                }
                "scan_parallelism" => {
                    let n: usize = match value.parse() {
                        Ok(n) => n,
                        Err(err) => {
                            return Err(CommandConfigError::InvalidScanParallelismValue(err))
                        }
                    };
                    Config::ScanParallelism(n)
                }
                _ => return Err(CommandConfigError::InvalidKey(key.to_string())),
            };

            let query = "INSERT INTO config(key, value) VALUES($key, $value) ON CONFLICT(key) DO UPDATE SET value = excluded.value";

            db.execute(query, rusqlite::params![key, config])?;
        } else {
            let key = args.value_of("key").unwrap();
            if !Config::is_valid_key(key) {
                return Err(CommandConfigError::InvalidKey(key.to_string()));
            }

            let value_result: rusqlite::Result<String> =
                db.query_row("SELECT value FROM config WHERE key = $key", [key], |row| {
                    row.get(0)
                });

            let value = match value_result {
                Ok(value) => value,
                Err(err) => match err {
                    rusqlite::Error::QueryReturnedNoRows => {
                        return Err(CommandConfigError::NoValue(key.to_string()));
                    }
                    _ => return Err(CommandConfigError::SQLite(err)),
                },
            };

            println!("{} = \"{}\"", key, value);
        }
    } else {
        let mut stmt = db.prepare("SELECT key, value FROM config")?;
        let mut rows = stmt.query([])?;

        while let Some(row) = rows.next()? {
            let key: String = row.get(0)?;
            let value: String = row.get(1)?;

            println!("{} = \"{}\"", key, value);
        }
    }

    Ok(())
}

#[derive(Debug)]
enum CommandScanError {}

fn cmd_scan(
    db: &mut rusqlite::Connection,
    args: &clap::ArgMatches,
) -> Result<(), CommandScanError> {
    Ok(())
}
impl fmt::Display for CommandScanError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        Ok(())
    }
}

enum AppError {
    OpenDatabase(OpenDatabaseError),
    InitDatabase(InitDatabaseError),
    CommandConfig(CommandConfigError),
    CommandScan(CommandScanError),
}

impl fmt::Display for AppError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            AppError::OpenDatabase(err) => write!(f, "unable to open database: {}", err),
            AppError::CommandConfig(err) => write!(f, "{}", err),
            AppError::CommandScan(err) => write!(f, "{}", err),
            _ => write!(f, "foobar"),
        }
    }
}

impl From<OpenDatabaseError> for AppError {
    fn from(err: OpenDatabaseError) -> AppError {
        AppError::OpenDatabase(err)
    }
}
impl From<InitDatabaseError> for AppError {
    fn from(err: InitDatabaseError) -> AppError {
        AppError::InitDatabase(err)
    }
}
impl From<CommandConfigError> for AppError {
    fn from(err: CommandConfigError) -> AppError {
        AppError::CommandConfig(err)
    }
}
impl From<CommandScanError> for AppError {
    fn from(err: CommandScanError) -> AppError {
        AppError::CommandScan(err)
    }
}

fn do_main(matches: &clap::ArgMatches) -> Result<(), AppError> {
    let mut database = open_database()?;
    init_database(&mut database)?;

    match matches.subcommand() {
        Some(("config", sub_matches)) => {
            cmd_config(&mut database, sub_matches)?;
        }
        Some(("scan", sub_matches)) => {
            cmd_scan(&mut database, sub_matches)?;
        }
        _ => unreachable!("should never happen"),
    }

    Ok(())
}

fn main() {
    let matches = Command::new("zik")
        .author("Vincent Rischmann <vincent@rischmann.fr>")
        .version("1.0")
        .about("Create a database of your music library")
        .subcommand(
            Command::new("config")
                .about("View or set the configuration")
                .arg(Arg::new("key").takes_value(true).required(false))
                .arg(Arg::new("value").takes_value(true).required(false)),
        )
        .subcommand(Command::new("scan").about("Scan your music library"))
        .get_matches();

    if let Err(err) = do_main(&matches) {
        println!("{}", err)
    }
}
