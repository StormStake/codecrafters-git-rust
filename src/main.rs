use clap::Parser;
use clap::Subcommand;
use sha1::Digest;
use sha1::Sha1;
#[allow(unused_imports)]
use std::env;
#[allow(unused_imports)]
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;

/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    name: Option<String>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    Init {},
    LsTree {
        #[clap(long, short)]
        name_only: bool,
        object_sha: String,
    },
    CatFile {
        #[clap(long, short)]
        pretty_print: bool,
        object_sha: String,
    },
    HashObject {
        #[clap(long, short)]
        write: bool,
        file: String,
    },
    WriteTree {},
}

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    let args = Args::parse();
    match args.command {
        Command::Init {} => {
            fs::create_dir(".git").unwrap();
            fs::create_dir(".git/objects").unwrap();
            fs::create_dir(".git/refs").unwrap();
            fs::write(".git/HEAD", "ref: refs/heads/main\n").unwrap();
            println!("Initialized git directory");
        }
        Command::CatFile {
            pretty_print: _,
            object_sha,
        } => {
            let file_data = fs::read(format!(
                "./.git/objects/{}/{}",
                &object_sha[..2],
                &object_sha[2..]
            ))
            .expect("Failed to open file");

            let z = flate2::read::ZlibDecoder::new(&file_data[..]);

            let mut object_header = vec![];
            let mut zreader = io::BufReader::new(z);
            let _n = zreader
                .read_until(0, &mut object_header)
                .expect("Failed to read null byte");

            let header = String::from_utf8(object_header).expect("Failed to read utf-8");
            let _object_type = header
                .split_whitespace()
                .next()
                .expect("Failed to find object header");

            let nfile_bytes = header
                .split_whitespace()
                .last()
                .expect("Failed to find file length")
                .strip_suffix("\0")
                .expect("No null byte found");

            let nfile_bytes = nfile_bytes.parse().expect("File length was not an integer");

            let mut object_data = vec![0; nfile_bytes];
            zreader
                .read_exact(&mut object_data)
                .expect(&format!("Failed to read {} bytes", nfile_bytes));
            print!(
                "{}",
                String::from_utf8(object_data).expect("Failed to parse file data")
            );
        }
        Command::LsTree {
            name_only,
            object_sha,
        } => {
            let file_data = fs::read(format!(
                "./.git/objects/{}/{}",
                &object_sha[..2],
                &object_sha[2..]
            ))
            .expect("Failed to open file");

            let z = flate2::read::ZlibDecoder::new(&file_data[..]);
            let mut object_header = vec![];
            let mut zreader = io::BufReader::new(z);

            let _n = zreader
                .read_until(0, &mut object_header)
                .expect("Failed to read null byte");

            let _header = String::from_utf8(object_header).expect("Failed to read utf-8");
            loop {
                let mut meta_data = vec![];
                if let Err(_n) = zreader.read_until(b'\0', &mut meta_data) {
                    break;
                };
                if meta_data.len() == 0 {
                    break;
                }
                meta_data.pop().unwrap();

                let meta = String::from_utf8(meta_data).unwrap();

                let mut meta_split = meta.split(" ");
                let object_mode = meta_split.next().expect("Failed to find mode");
                let object_name = meta_split.next().expect("Failed to find name");
                let object_type = match object_mode.starts_with("40000") {
                    true => "tree",
                    false => "blob",
                };

                let mut sha_data = vec![0; 20];
                let _ = zreader.read_exact(&mut sha_data);
                let mut sha_hex = "".to_string();
                for byte in sha_data {
                    sha_hex += &format!("{:02x}", byte);
                }

                if name_only {
                    println!("{object_name}");
                } else {
                    println!("{object_mode} {object_type} {object_sha}\t{object_name}");
                }
            }
        }
        Command::HashObject { write, file } => {
            let mut object_data = fs::read(file).expect("Failed reading file");

            let size = object_data.len();

            let size_repr = size.to_string();

            let mut block: Vec<u8> = vec![];
            block.append(&mut "blob ".as_bytes().to_vec());
            block.append(&mut size_repr.into_bytes());
            block.push(0);
            block.append(&mut object_data);

            let mut hasher = Sha1::new();
            hasher.update(block.clone());
            let res = hasher.finalize();
            let mut object_sha = "".to_string();

            for byte in res {
                print!("{byte:02x?}");
                object_sha += &format!("{byte:02x?}");
            }
            println!("");

            if write {
                let _ = fs::create_dir(format!(".git/objects/{}", &object_sha[..2]));
                let file = fs::File::create(format!(
                    "./.git/objects/{}/{}",
                    &object_sha[..2],
                    &object_sha[2..]
                ))
                .expect("Failed to open file");
                let mut zwriter =
                    flate2::write::ZlibEncoder::new(file, flate2::Compression::new(1));
                let _n = zwriter.write_all(&block).expect("Failed to write to file");
            }
        }

        Command::WriteTree {} => {
            let tree_ent = write_tree(std::env::current_dir().unwrap(), true);

            let hash: Vec<&u8> = tree_ent
                .iter()
                .skip_while(|b| **b != b'\0')
                .skip(1)
                .take(20)
                .collect();

            for byte in hash {
                print!("{byte:02x}");
            }
            println!("");
        }
    }
}

fn write_blob(file: PathBuf, write: bool) -> Vec<u8> {
    let mut object_data = fs::read(&file).expect("Failed reading file");

    let size = object_data.len();

    let size_repr = size.to_string();

    let mut block: Vec<u8> = vec![];
    block.append(&mut "blob ".as_bytes().to_vec());
    block.append(&mut size_repr.into_bytes());
    block.push(0);
    block.append(&mut object_data);

    let mut hasher = Sha1::new();

    hasher.update(block.clone());
    let res = hasher.finalize();
    let mut object_sha = "".to_string();
    let mut hash = vec![];
    for byte in res {
        hash.push(byte);
        object_sha += &format!("{byte:02x?}");
    }

    if write {
        let _ = fs::create_dir(format!(".git/objects/{}", &object_sha[..2]));
        let file = fs::File::create(format!(
            "./.git/objects/{}/{}",
            &object_sha[..2],
            &object_sha[2..]
        ))
        .expect("Failed to open file");
        let mut zwriter = flate2::write::ZlibEncoder::new(file, flate2::Compression::new(1));
        let _n = zwriter.write_all(&block).expect("Failed to write to file");
    }

    let metadata = fs::metadata(&file).unwrap();

    let mode = if metadata.permissions().mode() & 0o111 == 0 {
        "100644"
    } else {
        "100755"
    };
    let name = file.file_name().unwrap().to_str().unwrap();
    let mut preface = format!("{mode} {name}").into_bytes();
    preface.push(b'\0');
    preface.append(&mut hash);
    preface
}

fn write_tree(path: PathBuf, write: bool) -> Vec<u8> {
    let dir_ents = fs::read_dir(&path).expect("Failed to get dir ents from tree");
    let mut output = vec![];
    for ent in dir_ents {
        let repr;
        let ent = ent.unwrap();

        let ft = ent.file_type().unwrap();

        if ft.is_dir() {
            let name = ent.file_name();
            if name == ".git" {
                continue;
            }
            repr = write_tree(ent.path(), true);
        } else {
            repr = write_blob(ent.path(), true);
        }

        output.push(repr);
    }

    output.sort_by_key(|input| {
        let mut key = "".to_string();
        for byte in input.iter().skip_while(|b| **b != b' ') {
            key += &byte.to_string();
        }
        key
    });

    let mut object_data = output.concat();
    let n = object_data.len();
    let mut object_header: Vec<u8> = format!("tree {n}").bytes().collect();
    object_header.push(b'\0');
    let mut hasher = Sha1::new();

    object_header.append(&mut object_data);
    let object = object_header;
    hasher.update(&object);
    let res = hasher.finalize();
    let mut hash = vec![];
    let mut object_sha = "".to_string();
    for byte in res {
        hash.push(byte);
        object_sha += &format!("{byte:02x?}");
    }

    if write {
        let _ = fs::create_dir(format!(".git/objects/{}", &object_sha[..2]));
        let file = fs::File::create(format!(
            "./.git/objects/{}/{}",
            &object_sha[..2],
            &object_sha[2..]
        ))
        .expect("Failed to open file");
        eprintln!("Wrote file {:?}",object_sha);
        let mut zwriter = flate2::write::ZlibEncoder::new(file, flate2::Compression::new(1));
        let _n = zwriter.write_all(&object).expect("Failed to write to file");
    };

    let file_name = path.file_name().unwrap().to_str().unwrap();

    // Tree format ret
    let mut preface: Vec<u8> = format!("40000 {file_name}").bytes().collect();
    preface.push(b'\0');
    preface.append(&mut hash);
    preface
}
