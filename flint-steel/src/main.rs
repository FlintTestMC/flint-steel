use dotenvy::dotenv;
use flint_core::index::Index;
use flint_core::loader::TestLoader;
use std::env;
use std::path::{Path, PathBuf};

const TEST_PATH: &str = "./test";

fn main() {
    dotenv().ok();
    let args: Vec<String> = env::args().collect();
    let test_paths: Vec<PathBuf>;
    if args.len() > 1 {
        match args[1].as_str() {
            "index" => {
                println!("index");
                match TestLoader::new(Path::new(TEST_PATH), true) {
                    Ok(mut loader) => match loader.collect_all_test_files() {
                        Ok(test_files) => {
                            if let Err(err) = loader.get_index().generate_index(&test_files) {
                                eprintln!("{}", err);
                            }
                        }
                        Err(err) => {
                            eprintln!("{}", err);
                        }
                    },
                    Err(err) => {
                        println!("error while loading test files: {}", err);
                    }
                }
                return;
            }
            _ => {
                println!("Will run tests on a specific scope");
                match TestLoader::new(Path::new(TEST_PATH), true) {
                    Ok(loader) => match loader.collect_by_tags(&args[1..]) {
                        Ok(_test_paths) => {
                            test_paths = _test_paths;
                        }
                        Err(err) => {
                            println!("error while loading test files: {}", err);
                        }
                    },
                    Err(err) => {
                        println!("error while loading test files: {}", err);
                    }
                }
            }
        }
    } else {
        // Loads all test from the index
        println!("Will run all tests");
        match TestLoader::new(Path::new(TEST_PATH), true) {
            Ok(loader) => match loader.collect_all_test_files() {
                Ok(_test_paths) => {
                    test_paths = _test_paths;
                }
                Err(err) => {
                    println!("error while loading test files: {}", err);
                }
            },
            Err(err) => {
                println!("error while loading test files: {}", err);
            }
        }
    }
}
