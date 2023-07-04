use chrono::{DateTime, FixedOffset};
use clap::{arg, Command};
use regex::Regex;
use rusqlite::{Connection, Result};
use rust_stemmers::{Algorithm, Stemmer};
use std::error::Error;
use std::{env, io};
use uuid::Uuid;

struct Snip {
    uuid: String,
    name: String,
    text: String,
    timestamp: DateTime<FixedOffset>,
}

fn main() -> Result<(), Box<dyn Error>> {
    let cmd = Command::new("snip-rs")
        .bin_name("snip-rs")
        .arg_required_else_help(true)
        .subcommand_required(true)
        .subcommand(clap::command!("ls").about("List all snips"))
        .subcommand(
            clap::command!("split")
                .about("Split a string into words")
                .arg(arg!([string] "The string to split"))
                .arg_required_else_help(false),
        )
        .subcommand(
            Command::new("stem")
                .about("Stem word from stdin")
                .arg(arg!(<word> "The word to stem"))
                .arg_required_else_help(true),
        )
        .subcommand(clap::command!("get").about("Print first snip in database"));

    let matches = cmd.get_matches();

    let db_file_default = ".snip.sqlite3".to_string();
    let home_dir = match env::var("HOME") {
        Ok(v) => v,
        Err(e) => panic!("{}", e),
    };
    let db_path = env::var("SNIP_DB").unwrap_or(format!("{}/{}", home_dir, db_file_default));
    let conn = Connection::open(db_path)?;

    // process all subcommands as in: https://docs.rs/clap/latest/clap/_derive/_cookbook/git/index.html
    match matches.subcommand() {
        Some(("get", _)) => {
            let s = match get_first_snip(&conn) {
                Ok(v) => v,
                Err(e) => panic!("{}", e),
            };
            println!(
                "first snip: uuid: {} timestamp: {} name: {} text: {}",
                s.uuid, s.timestamp, s.name, s.text
            );
        }
        Some(("help", _)) => {
            println!("help");
        }
        Some(("ls", _)) => {
            list_snips(&conn).expect("could not list snips");
        }
        Some(("stem", sub_matches)) => {
            let term = match sub_matches.get_one::<String>("word") {
                Some(v) => v.to_owned(),
                None => read_data_from_stdin()?,
            };
            println!("{} -> {}", term, stem_something(&term));
        }
        Some(("split", sub_matches)) => {
            let input = match sub_matches.get_one::<String>("string") {
                Some(v) => v.to_owned(),
                None => read_lines_from_stdin(),
            };
            println!("{:?}", split_words(&input));
        }
        _ => {
            println!("invalid subcommand");
        }
    }

    Ok(())
}

fn read_lines_from_stdin() -> String {
    let mut buf = String::new();
    let mut data = String::new(); 

    let mut bytes_read;
    loop {
        bytes_read = io::stdin().read_line(&mut buf);

        match bytes_read {
            Ok(v) => {
                match v {
                    v if v > 0 => data = data + &buf.to_owned(),
                    _ => break,
                }
            }
            Err(_) => break,
        }
    }
    data
}

fn split_words(s: &str) -> Vec<&str> {
    let input = s.trim_start().trim_end();

    let pattern = Regex::new(r"(?m)\s+").unwrap();
    let words = pattern.split(input);
    let mut output: Vec<&str> = Vec::new();
    for w in words.into_iter() {
        output.push(strip_punctuation(w));
    }
    output
}

fn strip_punctuation(s: &str) -> &str{
    let chars_strip = &['.', ',', '!', '?', '"', '[', ']', '(', ')'];

    let mut clean = match s.strip_prefix(chars_strip) {
        Some(v) => v,
        None => s,
    };
    clean = match clean.strip_suffix(chars_strip) {
        Some(v) => v,
        None => clean,
    };
    clean
}

fn get_first_snip(conn: &Connection) -> Result<Snip, Box<dyn Error>> {
    let mut stmt = match conn.prepare("SELECT uuid, name, timestamp, data FROM snip LIMIT 1") {
        Ok(v) => v,
        Err(e) => return Err(Box::new(e)),
    };

    let mut query_iter = stmt.query_map([], |row| {
        // parse timestamp
        let ts: String = row.get(2)?;
        let ts_parsed = match DateTime::parse_from_rfc3339(ts.as_str()) {
            Ok(v) => v,
            Err(e) => panic!("{}", e),
        };

        Ok(Snip {
            uuid: row.get(0)?,
            name: row.get(1)?,
            timestamp: ts_parsed,
            text: row.get(3)?,
        })
    })?;

    if let Some(s) = query_iter.next() {
        return Ok(s.unwrap());
    }

    Err(Box::new(std::io::Error::new(
        io::ErrorKind::NotFound,
        "damn",
    )))
}

fn list_snips(conn: &Connection) -> Result<(), Box<dyn Error>> {
    let mut stmt = match conn.prepare("SELECT uuid, name, timestamp, data from snip") {
        Ok(v) => v,
        Err(e) => panic!("{}", e),
    };

    let query_iter = stmt.query_map([], |row| {
        // parse timestamp
        let ts: String = row.get(2)?;
        let ts_parsed = match DateTime::parse_from_rfc3339(ts.as_str()) {
            Ok(v) => v,
            Err(e) => panic!("{}", e),
        };

        Ok(Snip {
            uuid: row.get(0)?,
            name: row.get(1)?,
            timestamp: ts_parsed,
            text: row.get(3)?,
        })
    })?;

    for snip in query_iter {
        let s = snip.unwrap();
        let id = Uuid::parse_str(&s.uuid)?;

        println!("{} {} {}", split_uuid(id)[0], s.timestamp, s.name);
    }

    Ok(())
}

fn stem_something(s: &str) -> String {
    let stemmer = Stemmer::create(Algorithm::English);
    stemmer.stem(s.to_lowercase().as_str()).to_string()
}

fn read_data_from_stdin() -> Result<String, io::Error> {
    let mut buffer = String::new();
    io::stdin().read_line(&mut buffer)?;
    Ok(buffer.trim_end().to_owned())
}

fn split_uuid(uuid: Uuid) -> Vec<String> {
    uuid.to_string().split('-').map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn split_mutiline_string() -> Result<()> {
        let s = r#"Lorem ipsum (dolor) sit amet, consectetur
second line?

that was an [empty] line.
"#;
        let expect: Vec<&str> = vec![
            "Lorem",
            "ipsum",
            "dolor",
            "sit",
            "amet",
            "consectetur",
            "second",
            "line",
            "that",
            "was",
            "an",
            "empty",
            "line",
        ];
        let split = split_words(s);
        assert_eq!(expect, split);
        Ok(())
    }
}