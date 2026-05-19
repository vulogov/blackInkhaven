extern crate log;
use std::fs;
use std::io::{self, BufRead};
use curl::easy::{Easy2, Handler, WriteError};

#[time_graph::instrument]
pub fn get_file_from_stdin() -> String {
    let mut lines = io::stdin().lock().lines();
    let mut user_input = String::new();

    while let Some(line) = lines.next() {
        let last_input = line.unwrap();

        // stop reading
        if last_input.len() == 0 {
            user_input.push_str("\n");
        }

        // add a new line once user_input starts storing user input
        if user_input.len() > 0 {
            user_input.push_str("\n");
        }

        // store user input
        user_input.push_str(&last_input);
    }
    log::debug!("INPUT: get {} bytes from stdin", &user_input.len());
    user_input.clone()
}

#[time_graph::instrument]
pub fn get_file_from_uri(some_url: String) -> Option<String>  {
    struct Collector(Vec<u8>);

    impl Handler for Collector {
    fn write(&mut self, data: &[u8]) -> Result<usize, WriteError> {
            self.0.extend_from_slice(data);
            Ok(data.len())
        }
    }

    let mut easy = Easy2::new(Collector(Vec::new()));
    let _ = easy.useragent("ZBUS");
    easy.get(true).unwrap();
    easy.url(&some_url).unwrap();
    match easy.perform() {
        Err(err) => {
            log::error!("Request from {} returns {}", some_url, err);
            return None;
        }
        _ => {}
    }
    let contents = easy.get_ref();
    Some(String::from_utf8_lossy(&contents.0).to_string())
}

pub fn get_file_from_file(full_path: String) -> Option<String> {
    get_file_from_uri(format!("file://{}", &full_path.as_str()))
}

pub fn get_file_from_relative_file(full_path: String) -> Option<String> {
    match fs::read_to_string(full_path) {
        Ok(data) => Some(data),
        Err(_) => None,
    }
}

pub fn get_snippet(source_stdin: bool, source_eval: Option<String>, source_file: Option<String>, source_url: Option<String>) -> Option<String> {
    let snippet = if source_stdin {
        get_file_from_stdin()
    } else {
        match source_eval {
            Some(snippet) => snippet.to_string(),
            None => {
                match source_file {
                    Some(snippet_file) => {
                        let snippet = match get_file_from_file(snippet_file.to_string()) {
                            Some(snippet) => snippet,
                            None =>  {
                                log::error!("EError getting snippet from file: {}", &snippet_file.to_string());
                                return None;
                            }
                        };
                        snippet
                    }
                    None => {
                        match source_url {
                            Some(snippet_uri) => {
                                let snippet = match get_file_from_uri(snippet_uri.to_string()) {
                                    Some(snippet) => snippet,
                                    None => {
                                        log::error!("EError getting snippet from URL: {}", &snippet_uri.to_string());
                                        return None;
                                    }
                                };
                                snippet
                            }
                            None => {
                                log::error!("No sources for script were provided");
                                return None;
                            }
                        }
                    }
                }
            }
        }
    };
    return Some(snippet);
}
