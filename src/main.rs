use clap::Parser;
use clap::Subcommand;
use sha1::Digest;
use sha1::Sha1;
use std::collections::HashMap;
#[allow(unused_imports)]
use std::env;
#[allow(unused_imports)]
use std::fs;
use std::fs::File;
use std::io;
use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::path::PathBuf;

mod dewey;

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
    Debug {},
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
    CommitTree {
        tree_sha: String,
        #[clap(short)]
        parent_sha: String,
        #[clap(long, short)]
        message: String,
    },
    Clone {
        url: String,
    },
}

fn init(mut filepath: PathBuf) {
    fs::create_dir(filepath.clone()).unwrap();
    filepath.push(".git");
    fs::create_dir(&filepath).unwrap();
    let mut objects_path = filepath.clone();
    objects_path.push("objects");
    fs::create_dir(objects_path).unwrap();
    let mut refs_path = filepath.clone();
    refs_path.push("refs");
    fs::create_dir(refs_path).unwrap();
    let mut head_path = filepath.clone();
    head_path.push("HEAD");
    fs::write(head_path, "ref: refs/heads/main\n").unwrap();
    println!("Initialized git directory");
}

fn main() {
    // You can use print statements as follows for debugging, they'll be visible when running tests.
    let args = Args::parse();
    match args.command {
        Command::Init {} => {
            init(std::env::current_dir().unwrap());
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
        Command::CommitTree {
            tree_sha,
            parent_sha,
            message,
        } => {
            // println!("Tree: {tree_sha}");
            // println!("Parent: {parent_sha}");
            // println!("Message: {message}");
            write_commit(tree_sha, parent_sha, message);
        }
        Command::Clone { url } => {
            let (main, items) = clone(url.clone());
            println!("HEAD should be {main}");
            let mut repo_name = (url).split("/").last().unwrap();
            repo_name = repo_name.strip_suffix(".git").unwrap_or(repo_name);
            init(repo_name.into());
            for (_, object_sha, data) in items.values() {
                let _ = fs::create_dir(format!("{repo_name}/.git/objects/{}", &object_sha[..2]));

                let file = fs::File::create(format!(
                    "{repo_name}/.git/objects/{}/{}",
                    &object_sha[..2],
                    &object_sha[2..]
                )).unwrap();
                let mut zwriter =
                    flate2::write::ZlibEncoder::new(file, flate2::Compression::new(1));
                let _n = zwriter.write_all(&data).expect("Failed to write to file");
            }
        }
        Command::Debug {} => {
            let delta_buf = fs::read("./test.delta").unwrap();
            let base_buf = fs::read("./base.data").unwrap();
            let new = dedelta_buf(delta_buf, base_buf);

            println!("{new:?}");
        }
    }
}

fn write_commit(tree: String, parent: String, message: String) -> String {
    let tree_line = format!("tree {tree}\n");
    let parent_line = format!("parent {parent}\n");
    let author_line = format!("author Aaron <email.email@gmail.com> 10 -0500\n");
    let commiter_line =
        format!("committer aaron <aaron@localhost.localdomain> 1727570086 -0500\n\n");
    let message = format!("{message}");
    let len = tree_line.len()
        + parent_line.len()
        + author_line.len()
        + commiter_line.len()
        + message.len();
    let len = len.to_string();

    let content = format!("{tree_line}{parent_line}{author_line}{commiter_line}{message}\n");

    let mut commit_bytes = format!("commit {len}").to_string().into_bytes();
    commit_bytes.push(0);
    let mut content_bytes = content.into_bytes();
    commit_bytes.append(&mut content_bytes);

    let mut hasher = Sha1::new();

    hasher.update(commit_bytes.clone());
    let res = hasher.finalize();
    let mut object_sha = "".to_string();
    let mut hash = vec![];
    for byte in res {
        hash.push(byte);
        object_sha += &format!("{byte:02x?}");
    }

    let _ = fs::create_dir(format!(".git/objects/{}", &object_sha[..2]));
    let file = fs::File::create(format!(
        "./.git/objects/{}/{}",
        &object_sha[..2],
        &object_sha[2..]
    ))
    .expect("Failed to open file");
    let mut zwriter = flate2::write::ZlibEncoder::new(file, flate2::Compression::new(1));
    let _n = zwriter
        .write_all(&commit_bytes)
        .expect("Failed to write to file");
    println!("{object_sha}");
    object_sha
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
        eprintln!("Wrote file {:?}", object_sha);
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

fn clone(url: String) -> (String, HashMap<usize, (ObjectType, String, Vec<u8>)>) {
    let service_url = url.clone() + "/info/refs?service=git-upload-pack";
    let upload_url = url + "/git-upload-pack";
    let client = reqwest::blocking::Client::new();
    let resp = client.get(service_url).send().unwrap();
    let binding = resp.bytes().unwrap();
    let mut bytes = binding.into_iter();
    let mut end = false;
    let mut cap = false;
    let mut ref_hash = String::new();
    // Read ref advert
    loop {
        let len: Vec<u8> = bytes.by_ref().take(4).collect();
        let len = usize::from_str_radix(&String::from_utf8(len).unwrap(), 16).unwrap();

        if len == 0 {
            if end {
                break;
            } else {
                end = true;
            }
            continue;
        }

        let content: Vec<u8> = bytes.by_ref().take(len - 4).collect();
        if len < 63 {
            let comment = String::from_utf8(content).unwrap();
            continue;
        }
        if !cap {
            let nul = content.iter().position(|n| *n == 0).unwrap();

            let mut cap_ref = String::new();
            content.take(nul as u64).read_to_string(&mut cap_ref);

            let caps: Vec<u8> = content.into_iter().skip(nul).collect();
            let caps = String::from_utf8(caps).unwrap();
            cap = true;
        } else {
            let ref_line = String::from_utf8(content).unwrap();

            ref_hash = ref_line.split(" ").take(1).collect();
        }
    }
    let mut body: Vec<u8> = vec![];
    let mut eom = "0000".bytes().collect();
    let mut want = format!("0077want {ref_hash} multi_ack_detailed side-band-64k thin-pack ofs-delta agent=git/1.8.2\n").bytes().collect();
    let mut done = "0009done\n".bytes().collect();

    body.append(&mut want);
    body.append(&mut eom);
    body.append(&mut done);

    let resp = client.post(upload_url).body(body).send().unwrap();
    let mut resp_data = resp.bytes().unwrap().into_iter();
    let len_data: Vec<u8> = resp_data.by_ref().take(4).collect();
    let len = usize::from_str_radix(&String::from_utf8(len_data).unwrap(), 16).unwrap();

    let data: Vec<u8> = resp_data.by_ref().take(len - 4).collect();

    let mut file = File::create("./tmp.pack").unwrap();

    loop {
        let len_data: Vec<u8> = resp_data.by_ref().take(4).collect();
        let len = usize::from_str_radix(&String::from_utf8(len_data).unwrap(), 16).unwrap();
        if len == 0 {
            break;
        }
        let type_val = resp_data.by_ref().next().unwrap();
        let data: Vec<u8> = resp_data.by_ref().take(len - 5).collect();
        match type_val {
            1 => {
                file.write(data.as_slice());
            }
            2 => {}
            3 => {}
            _ => {
                panic!("Wrong type")
            }
        }
    }

    file.flush();
    drop(file);
    let mut pack_data = vec![];
    let mut file = File::open("./tmp.pack").unwrap();
    file.read_to_end(&mut pack_data).unwrap();

    (ref_hash, parse_pack(pack_data))
}

enum ObjectType {
    Blob,
    Commit,
    Tree,
    Tag,
}

fn parse_pack(pack_data: Vec<u8>) -> HashMap<usize, (ObjectType, String, Vec<u8>)> {
    let mut pack_items: HashMap<usize, (ObjectType, String, Vec<u8>)> = HashMap::new();

    let mut cursor = 0;
    let mut pack_data = pack_data.into_iter();
    let pack: Vec<u8> = pack_data.by_ref().take(4).collect();
    cursor += 4;
    let version: Vec<u8> = pack_data.by_ref().take(4).collect();
    cursor += 4;
    let n_obj_1 = pack_data.by_ref().next().unwrap();
    let n_obj_2 = pack_data.by_ref().next().unwrap();
    let n_obj_3 = pack_data.by_ref().next().unwrap();
    let n_obj_4 = pack_data.by_ref().next().unwrap();
    cursor += 4;
    let n_obj = u32::from_le_bytes([n_obj_4, n_obj_3, n_obj_2, n_obj_1]);

    loop {
        let obj_pos = cursor;
        let (n, mut item_type, size) = dewey::pack_item(&mut pack_data);
        cursor += n;
        let mut sha_bytes: Vec<u8> = vec![];
        let mut ofs: u128 = 0;

        let mut base_data = vec![];
        let mut base_hash = String::new();
        let target_type = match item_type {
            1..=5 => item_type,

            6 => {
                // Init: len MSB + 7bit Size
                let (dofs_n, dofs_ofs) = dewey::delta_offset(&mut pack_data);
                cursor += dofs_n;
                ofs = dofs_ofs;

                let base_pos = obj_pos as u128 - ofs;

                let (base_type, hash, data) = pack_items.get(&(base_pos as usize)).unwrap();
                base_data = data.to_vec();
                base_hash = hash.to_string();
                match base_type {
                    ObjectType::Commit => 1,
                    ObjectType::Tree => 2,
                    ObjectType::Blob => 3,
                    ObjectType::Tag => 4,
                }
            }
            7 => {
                sha_bytes = pack_data.by_ref().take(20).collect();
                cursor += 20;
                //Get base object data and type
                1
            }
            _ => {
                panic!();
            }
        };

        let bind = pack_data.collect::<Vec<u8>>();
        let mut zlib = flate2::bufread::ZlibDecoder::new(bind.as_slice());

        let mut d = vec![];
        let header_len = cursor - obj_pos;

        // Reading object data
        match zlib.read_to_end(&mut d) {
            Ok(_) => {}
            Err(_) => {
                break;
            }
        };

        // Dedelta if delta type
        if item_type == 6 || item_type == 7 {
            d = dedelta_buf(d, base_data);
        };

        // Writing lengths
        let mut in_c = zlib.total_in();
        cursor += in_c;
        let out_c = zlib.total_out();

        // Generating object hash
        let mut object_data: Vec<u8> = vec![];
        let type_header = match target_type {
            1 => "commit",
            2 => "tree",
            3 => "blob",
            4 => "tag",
            _ => panic!(),
        };
        let length_string = out_c.to_string();

        object_data.append(&mut type_header.bytes().collect());
        object_data.push(b' ');

        if item_type == 6 || item_type == 7 {
            object_data.append(&mut d.len().to_string().bytes().collect());
        } else {
            object_data.append(&mut length_string.bytes().collect());
        }
        object_data.push(0);
        object_data.append(&mut (d.clone()));

        let mut hasher = Sha1::new();
        hasher.update(object_data);
        let res = hasher.finalize();
        let mut object_sha = "".to_string();

        for byte in res {
            object_sha += &format!("{byte:02x?}");
        }

        let ty = match target_type {
            1 => ObjectType::Commit,
            2 => ObjectType::Tree,
            3 => ObjectType::Blob,
            4 => ObjectType::Tag,
            _ => panic!(),
        };

        pack_items.insert(obj_pos as usize, (ty, object_sha.clone(), d));
        // Returning the rest of the iterator
        pack_data = zlib.into_inner().to_vec().into_iter();
        match item_type {
            1..5 => {
                // Add extra length of object for length header
                in_c += n;

                println!("{object_sha} {type_header}\t{size} {in_c} {obj_pos}")
            }
            6 => {
                println!("{object_sha} {type_header}\t{size} {in_c} {obj_pos} {base_hash}");
            }
            7 => {
                let mut s = String::new();
                for b in sha_bytes {
                    s += &format!("{b:02x}");
                }
                println!("Type full ref, Obj: {s}, Size: {size}, Bytes read: {in_c}")
            }
            _ => panic!(),
        }
    }
    pack_items
}

fn dedelta_buf(delta_buf: Vec<u8>, base_buf: Vec<u8>) -> Vec<u8> {
    let mut delta_stream = &mut delta_buf.into_iter();
    let mut new_buf = vec![];

    let (_n1, size1) = dewey::delta_buf_length(&mut delta_stream);
    let (_n2, size2) = dewey::delta_buf_length(&mut delta_stream);
    // Byte pos counter
    let n = 0;

    while new_buf.len() < size2 as usize {
        let instruction = delta_stream.next().unwrap();
        let ins_type = instruction & 128;

        match ins_type {
            // Insert
            0 => {
                let n_bytes = instruction & 127;
                let mut bytes = delta_stream.take(n_bytes.into()).collect();
                new_buf.append(&mut bytes);
            }
            // Copy
            128 => {
                let (src_ofs, n_copy) = dewey::delta_copy_length(delta_stream, instruction);

                let bytes = &base_buf[src_ofs as usize..(src_ofs + n_copy) as usize];

                new_buf.extend_from_slice(bytes);
            }
            _ => panic!(),
        }
    }

    new_buf
}
