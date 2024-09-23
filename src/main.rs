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
                let object_type = match object_mode.starts_with("040000") {
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
    }
}
