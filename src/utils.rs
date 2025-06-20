use std::path::Path;
use std::process::Command;
use ansi_term::Colour::Red;

pub fn command_exists(command: &str) -> bool {
    if let Ok(path) = std::env::var("PATH") {
        for p in path.split(':') {
            let full_path = Path::new(p).join(command);
            if full_path.exists() {
                return true;
            }
        }
    }
    false
}

pub fn get_privilege_command() -> String {
    if command_exists("sudo") {
        "sudo".to_string()
    } else if command_exists("doas") {
        "doas".to_string()
    } else {
        eprintln!("{} Neither sudo nor doas found! Install one to continue.", 
                  Red.paint("Error:"));
        std::process::exit(1);
    }
}
