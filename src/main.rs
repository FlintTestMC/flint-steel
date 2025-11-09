mod json_data_structure;

use crate::json_data_structure::SimulationRun;
use serde_json::to_string_pretty;

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Write};
use std::path::Path;
use std::{env, io};

const TEST_PATH: &str = "./test";
fn main() -> io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let mut test_paths: Vec<String> = vec![];
    if args.len() > 1 {
        match args[1].as_str() {
            "index" => {
                println!("index");
                return generate_index(TEST_PATH);
            }
            _ => {
                println!("Will run tests on a specific scope");
                if !Path::new("index.json").exists() {
                    println!("Index does not exists, so need to build the index first");
                    generate_index(TEST_PATH)?;
                }
                let file = File::open("index.json")?;
                let reader = BufReader::new(file);
                let map: HashMap<String, String> =
                    serde_json::from_reader(reader).expect("broken index, isn't json!");
                let test = args[1].to_string();
                let is_dir = test.ends_with(":");
                for (k, v) in &map {
                    if (k.starts_with(&test) && is_dir) || *k == test {
                        println!("added test to current run: '{}' ", k);
                        test_paths.push(v.clone());
                    }
                }
            }
        }
    } else {
        // Loads all test from the index
        println!("Will run all tests");
        let map: BTreeMap<String, String>;
        if !Path::new("index.json").exists() {
            println!("Index does not exists, so need to build the index first");
            map = get_all_test_files(Path::new(TEST_PATH));
        } else {
            let file = File::open("index.json")?;
            let reader = BufReader::new(file);
            map = serde_json::from_reader(reader).expect("broken index, isn't json!");
        }
        for (k, v) in &map {
            println!("added test to current run: '{}' ", k);
            test_paths.push(v.clone());
        }
    }
    for path in test_paths {
        // Extract the data from the simulation run file
        let file = File::open(path)?;
        let reader = BufReader::new(file);
        let simulation_run: SimulationRun =
            serde_json::from_reader(reader).expect("broken test file, isn't json!");

        // TODO Do now the logic
    }
    Ok(())
}

fn get_all_test_files(start_dir: &Path) -> BTreeMap<String, String> {
    let mut dirs = vec![start_dir.to_path_buf()];
    let mut index: BTreeMap<String, String> = BTreeMap::new();

    while let Some(dir) = dirs.pop() {
        if let Ok(entries) = fs::read_dir(&dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    dirs.push(path);
                } else if let Some(ext) = path.extension() {
                    if ext == "json" {
                        if let Ok(rel_path) = path.strip_prefix(start_dir) {
                            let key = rel_path
                                .with_extension("")
                                .components()
                                .map(|c| c.as_os_str().to_string_lossy())
                                .collect::<Vec<_>>()
                                .join(":");
                            index.insert(key, path.to_str().unwrap().to_string());
                        }
                    }
                }
            }
        }
    }
    index
}

fn generate_index(path: &str) -> io::Result<()> {
    let start_dir = Path::new(path);
    if !start_dir.is_dir() {
        eprintln!("'{}' is no directory", start_dir.display());
        std::process::exit(1);
    }
    let index = get_all_test_files(start_dir);
    let index = to_string_pretty(&index).expect("hash_map for index is broken");
    let mut file = OpenOptions::new()
        .create(true).write(true)
        .open("index.json")?;
    file.write_all(index.as_bytes())?;
    Ok(())
}
