use std::fs;
use std::env;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::io::{self, Write};
use std::time::Instant;
use ansi_term::Colour::{Green, Red, Yellow};
use toml::{Value, Table};
use crate::utils;

pub fn install(packages: &[String], flags: &[String], yes: bool) {
    for package in packages {
        install_single(package, flags, yes);
    }
}

fn install_single(package: &str, flags: &[String], yes: bool) {
    if Path::new("/etc/arch-release").exists() {
        install_with_makepkg(package, flags, yes);
    } else {
        install_with_build_system(package, flags, yes);
    }
}

fn install_with_makepkg(package: &str, flags: &[String], yes: bool) {
    let start = Instant::now();
    let tmp = Path::new("/tmp/aurora");
    let builds = tmp.join("builds");

    for dir in [tmp, &builds] {
        if !dir.exists() {
            fs::create_dir_all(dir).expect("Failed to create temp directory");
        }
    }

    let build_dir = builds.join(package);

    if build_dir.exists() {
        fs::remove_dir_all(&build_dir).expect("Failed to clean previous build");
    }

    println!("\x1b[1m~> Cloning AUR repository: {}\x1b[0m", package);
    let status = Command::new("git")
        .arg("clone")
        .arg("--depth=1")
        .arg(format!("https://aur.archlinux.org/{}.git", package))
        .arg(&build_dir)
        .stdout(Stdio::null())
        .status()
        .expect("Git command failed");

    if !status.success() {
        eprintln!("{}", Red.paint("Failed to clone repository"));
        return;
    }

    println!("\x1b[1m~> Building and installing {} from AUR\x1b[0m", package);

    if !yes {
        println!("~> PKGBUILD contents:");
        let pkgbuild_path = build_dir.join("PKGBUILD");
        if pkgbuild_path.exists() {
            let status = Command::new("less")
                .arg(&pkgbuild_path)
                .status();

            if status.is_err() || !status.unwrap().success() {
                let _ = Command::new("cat")
                    .arg(&pkgbuild_path)
                    .status();
            }

            print!("~> Proceed with build? [Y/n] ");
            io::stdout().flush().unwrap();
            let mut input = String::new();
            io::stdin().read_line(&mut input).unwrap();

            if input.trim().eq_ignore_ascii_case("n") {
                println!("{}", Yellow.paint("Build cancelled by user"));
                return;
            }
        }
    }

    let mut makepkg = Command::new("makepkg");
    makepkg.arg("-si");

    if yes {
        makepkg.arg("--noconfirm");
    }

    for flag in flags {
        makepkg.arg(flag);
    }

    let status = makepkg
        .current_dir(&build_dir)
        .status()
        .expect("makepkg command failed");

    if !status.success() {
        eprintln!("{}", Red.paint("Build/install failed"));
        return;
    }

    println!("{} in {}s", Green.paint("~> INSTALL FINISHED"), start.elapsed().as_secs());
}

fn install_with_build_system(package: &str, flags: &[String], yes: bool) {
    let start = Instant::now();
    let tmp = Path::new("/tmp/aurora");
    let builds = tmp.join("builds");

    for dir in [tmp, &builds] {
        if !dir.exists() {
            fs::create_dir_all(dir).expect("Failed to create temp directory");
        }
    }

    let domain = "aur.archlinux.org";
    let repo = package;
    let build_dir = builds.join(repo);

    if build_dir.exists() {
        fs::remove_dir_all(&build_dir).expect("Failed to clean previous build");
    }

    println!("\x1b[1m~> Cloning repository: {}\x1b[0m", package);
    let status = Command::new("git")
        .arg("clone")
        .arg("--depth=1")
        .arg(format!("https://{}/{}.git", domain, package))
        .arg(&build_dir)
        .stdout(Stdio::null())
        .status()
        .expect("Git command failed");

    if !status.success() {
        eprintln!("{}", Red.paint("Failed to clone repository"));
        return;
    }

    println!("\x1b[1m~> Searching for build file\x1b[0m");
    let makefiles = ["Makefile", "makefile", "GNUMakefile"];
    let has_makefile = makefiles.iter().any(|f| build_dir.join(f).exists());

    let aurora_json_path = build_dir.join("aurora.json");
    let (build_system, custom_flags) = if aurora_json_path.exists() {
        parse_aurora_json(&aurora_json_path)
    } else if has_makefile {
        ("make".to_string(), vec![])
    } else if build_dir.join("configure").exists() {
        ("autotools".to_string(), vec![])
    } else if build_dir.join("Cargo.toml").exists() {
        ("cargo".to_string(), vec![])
    } else if build_dir.join("CMakeLists.txt").exists() {
        ("cmake".to_string(), vec![])
    } else if build_dir.join("meson.build").exists() {
        ("meson".to_string(), vec![])
    } else if build_dir.join("build.ninja").exists() {
        ("ninja".to_string(), vec![])
    } else if build_dir.join("*.nimble").exists() {
        ("nimble".to_string(), vec![])
    } else if build_dir.join("stack.yaml").exists() {
        ("stack".to_string(), vec![])
    } else {
        eprintln!("{}", Red.paint("No build system found"));
        return;
    };

    let mut final_flags = custom_flags;
    final_flags.extend(flags.iter().cloned());

    println!("~> Build system: {}", match build_system.as_str() {
        "make" => Green.paint("Make"),
        "autotools" => Green.paint("Autotools"),
        "cargo" => Green.paint("Cargo"),
        "cmake" => Green.paint("CMake"),
        "meson" => Green.paint("Meson"),
        "ninja" => Green.paint("Ninja"),
        "nimble" => Green.paint("Nimble"),
        "stack" => Green.paint("Stack"),
        _ => unreachable!()
    });

    let build_file = match build_system.as_str() {
        "make" => makefiles.iter().find(|f| build_dir.join(f).exists()).map(|f| f.to_string()),
        "autotools" => Some("configure".to_string()),
        "cargo" => Some("Cargo.toml".to_string()),
        "cmake" => Some("CMakeLists.txt".to_string()),
        "meson" => Some("meson.build".to_string()),
        "ninja" => Some("build.ninja".to_string()),
        "nimble" => build_dir.join("*.nimble").exists().then(|| "*.nimble".to_string()),
        "stack" => Some("stack.yaml".to_string()),
        _ => None,
    };

    if !yes {
        if let Some(file) = &build_file {
            let file_path = if file == "*.nimble" {
                let nimble_files: Vec<_> = fs::read_dir(&build_dir)
                    .unwrap()
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.extension().map(|e| e == "nimble").unwrap_or(false))
                    .collect();
                if nimble_files.is_empty() {
                    None
                } else {
                    nimble_files.get(0).cloned()
                }
            } else {
                Some(build_dir.join(file))
            };

            if let Some(file_path) = file_path {
                if file_path.exists() {
                    println!("~> Build file: {}", file);
                    let status = Command::new("less")
                        .arg(&file_path)
                        .status();

                    if status.is_err() || !status.unwrap().success() {
                        let _ = Command::new("cat")
                            .arg(&file_path)
                            .status();
                    }

                    print!("~> Proceed with build? [Y/n] ");
                    io::stdout().flush().unwrap();
                    let mut input = String::new();
                    io::stdin().read_line(&mut input).unwrap();

                    if input.trim().eq_ignore_ascii_case("n") {
                        println!("{}", Yellow.paint("Build cancelled by user"));
                        return;
                    }
                }
            }
        }
    }

    println!("~> Building with flags: {:?}", final_flags);
    let build_status = match build_system.as_str() {
        "make" => {
            let makefile = makefiles.iter()
                .find(|f| build_dir.join(f).exists())
                .unwrap_or(&"Makefile");

            let mut cmd = Command::new("make");
            cmd.arg("-f").arg(makefile)
                .args(&final_flags)
                .current_dir(&build_dir)
                .stdout(Stdio::null());

            cmd.status().expect("Make command failed")
        }
        "autotools" => {
            let configure_status = Command::new("./configure")
                .args(&final_flags)
                .current_dir(&build_dir)
                .status()
                .expect("Configure command failed");
            
            if !configure_status.success() {
                eprintln!("{}", Red.paint("Configure failed"));
                return;
            }
            
            Command::new("make")
                .current_dir(&build_dir)
                .stdout(Stdio::null())
                .status()
                .expect("Make command failed")
        }
        "cmake" => {
            let cmake_build_dir = build_dir.join("build");
            fs::create_dir_all(&cmake_build_dir).expect("Failed to create build dir");

            let mut cmake_cmd = Command::new("cmake");
            cmake_cmd
                .arg("-DCMAKE_BUILD_TYPE=Release")
                .args(&final_flags)
                .arg("..")
                .current_dir(&cmake_build_dir)
                .stdout(Stdio::null());

            let cmake_status = cmake_cmd.status();

            if cmake_status.is_err() || !cmake_status.as_ref().unwrap().success() {
                Command::new("cmake")
                    .args(&final_flags)
                    .arg("..")
                    .current_dir(&cmake_build_dir)
                    .stdout(Stdio::null())
                    .status()
                    .expect("CMake command failed")
            } else {
                cmake_status.unwrap()
            }
        }
        "cargo" => {
            let mut cargo_cmd = Command::new("cargo");
            cargo_cmd
                .arg("build")
                .arg("--release")
                .args(&final_flags)
                .arg("--manifest-path")
                .arg(build_dir.join("Cargo.toml"))
                .arg("--target-dir")
                .arg(build_dir.join("target"))
                .current_dir(&build_dir)
                .stdout(Stdio::null());

            cargo_cmd.status().expect("Cargo command failed")
        }
        "meson" => {
            let meson_build_dir = build_dir.join("build");
            fs::create_dir_all(&meson_build_dir).expect("Failed to create build dir");

            let meson_status = Command::new("meson")
                .arg("setup")
                .args(&final_flags)
                .arg(&meson_build_dir)
                .current_dir(&build_dir)
                .stdout(Stdio::null())
                .status();

            if meson_status.is_err() || !meson_status.as_ref().unwrap().success() {
                Command::new("meson")
                    .arg(&meson_build_dir)
                    .current_dir(&build_dir)
                    .stdout(Stdio::null())
                    .status()
                    .expect("Meson setup failed");
            }

            Command::new("ninja")
                .arg("-C")
                .arg(&meson_build_dir)
                .stdout(Stdio::null())
                .status()
                .expect("Ninja build failed")
        }
        "ninja" => {
            Command::new("ninja")
                .args(&final_flags)
                .current_dir(&build_dir)
                .stdout(Stdio::null())
                .status()
                .expect("Ninja build failed")
        }
        "nimble" => {
            Command::new("nimble")
                .arg("build")
                .args(&final_flags)
                .current_dir(&build_dir)
                .stdout(Stdio::null())
                .status()
                .expect("Nimble command failed")
        }
        "stack" => {
            Command::new("stack")
                .arg("install")
                .args(&final_flags)
                .arg("--local-bin-path")
                .arg(build_dir.join("bin"))
                .current_dir(&build_dir)
                .stdout(Stdio::null())
                .status()
                .expect("Stack command failed")
        }
        _ => unreachable!()
    };

    if !build_status.success() {
        eprintln!("{}", Red.paint("Build failed"));
        return;
    }

    println!("~> Installing...");
    let bin_path = find_binary_path(&build_dir, repo, &build_system);

    if bin_path.is_none() {
        eprintln!("{}: Failed to find built binary", Red.paint("Error"));
        return;
    }
    let bin_path = bin_path.unwrap();

    let home = env::var("HOME").expect("HOME environment variable not set");
    let dest = PathBuf::from(home).join(".local/bin");
    if !dest.exists() {
        fs::create_dir_all(&dest).expect("Failed to create local bin directory");
    }

    let bin_name = bin_path.file_name().unwrap().to_str().unwrap();
    let dest_path = dest.join(bin_name);

    fs::copy(&bin_path, &dest_path)
        .expect("Failed to copy binary to local directory");

    println!(
        "{}",
        Green.paint("Installed to ~/.local/bin. Make sure this directory is in your PATH.")
    );

    println!("{} in {}s", Green.paint("~> INSTALL FINISHED"), start.elapsed().as_secs());
}

fn parse_aurora_json(path: &Path) -> (String, Vec<String>) {
    let file = std::fs::File::open(path).expect("Failed to open aurora.json");
    let reader = std::io::BufReader::new(file);
    let json: serde_json::Value = serde_json::from_reader(reader).expect("Invalid aurora.json");

    let build_system = json["build_system"]
        .as_str()
        .unwrap_or("make")
        .to_string();

    let flags = json["flags"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default();

    (build_system, flags)
}

fn get_cargo_binary_name(build_dir: &Path) -> Option<String> {
    let cargo_toml = build_dir.join("Cargo.toml");
    let content = match fs::read_to_string(&cargo_toml) {
        Ok(c) => c,
        Err(_) => return None,
    };

    let value: Table = match content.parse() {
        Ok(v) => v,
        Err(_) => return None,
    };

    if let Some(bins) = value.get("bin") {
        if let Some(bin_array) = bins.as_array() {
            for bin_entry in bin_array {
                if let Some(bin_table) = bin_entry.as_table() {
                    if let Some(name) = bin_table.get("name") {
                        if let Some(name_str) = name.as_str() {
                            return Some(name_str.to_string());
                        }
                    }
                }
            }
        }
    }

    if let Some(package) = value.get("package") {
        if let Some(package_table) = package.as_table() {
            if let Some(name) = package_table.get("name") {
                if let Some(name_str) = name.as_str() {
                    return Some(name_str.to_string());
                }
            }
        }
    }

    None
}

fn find_binary_path(build_dir: &Path, repo: &str, build_system: &str) -> Option<PathBuf> {
    match build_system {
        "cargo" => {
            let binary_name = get_cargo_binary_name(build_dir).unwrap_or_else(|| repo.to_string());
            let release_path = build_dir.join("target/release").join(&binary_name);
            if release_path.exists() {
                return Some(release_path);
            }
            let debug_path = build_dir.join("target/debug").join(&binary_name);
            if debug_path.exists() {
                return Some(debug_path);
            }
            None
        },
        "make" | "autotools" | "ninja" => {
            let path = build_dir.join(repo);
            if path.exists() { Some(path) } else { None }
        },
        "cmake" => {
            let path = build_dir.join("build").join(repo);
            if path.exists() { Some(path) } else { None }
        },
        "meson" => {
            let build_output_dir = build_dir.join("build");
            find_executable_in_dir(&build_output_dir, repo)
        },
        "nimble" => {
            let path = build_dir.join(repo);
            if path.exists() { Some(path) } else { None }
        },
        "stack" => {
            let bin_dir = build_dir.join("bin");
            if bin_dir.exists() {
                find_executable_in_dir(&bin_dir, repo)
            } else {
                None
            }
        },
        _ => None
    }
}

fn find_executable_in_dir(dir: &Path, name: &str) -> Option<PathBuf> {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.is_dir() {
                if let Some(exec) = find_executable_in_dir(&path, name) {
                    return Some(exec);
                }
            } else if path.is_file() {
                if let Some(filename) = path.file_name() {
                    if filename == name {
                        return Some(path);
                    }
                }
            }
        }
    }
    None
}
