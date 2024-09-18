#[allow(unused_imports)]
use std::env;
#[allow(unused_imports)]
use std::fs;
use std::io;
use std::io::BufRead;
use std::io::Read;
use std::io::Write;
use sha1::Digest;
use sha1::Sha1;
use clap::Subcommand;
use clap::Parser;


/// Simple program to greet a person
#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
struct Args {
    /// Name of the person to greet
    #[arg(short, long)]
    name: Option<String>,

    #[command(subcommand)]
    command: Command
}

#[derive(Debug, Subcommand)]
enum Command
{
    Init {

    },

    CatFile {
        #[clap(long, short)]
        pretty_print: bool,
        object_sha: String
    },
    HashObject {
        #[clap(long, short)]
        write: bool,
        file: String
    }  
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
        },
        Command::CatFile { pretty_print, object_sha } => {
            
            
            let file_data = fs::read(format!("./.git/objects/{}/{}", &object_sha[..2],&object_sha[2..])).expect("Failed to open file");
            

            let z = flate2::read::ZlibDecoder::new(&file_data[..]);
            
            let mut object_header = vec![];
            let mut zreader = io::BufReader::new(z);
            let _n = zreader.read_until(0, &mut object_header).expect("Failed to read null byte");
            
            let header = String::from_utf8(object_header).expect("Failed to read utf-8");
            let object_type = header.split_whitespace().next().expect("Failed to find object header");
            
            
            let nfile_bytes = header.split_whitespace().last().expect("Failed to find file length").strip_suffix("\0").expect("No null byte found");
            
            let nfile_bytes = nfile_bytes.parse().expect("File length was not an integer");
            

            let mut object_data = vec![0; nfile_bytes];
            zreader.read_exact(&mut object_data).expect(&format!("Failed to read {} bytes",nfile_bytes));
            print!("{}", String::from_utf8(object_data).expect("Failed to parse file data"));
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
                fs::create_dir(format!(".git/objects/{}",&object_sha[..2]));
                let file = fs::File::create(format!("./.git/objects/{}/{}", &object_sha[..2],&object_sha[2..])).expect("Failed to open file");
                let mut zwriter = flate2::write::ZlibEncoder::new(file,flate2::Compression::new(1));
                let n = zwriter.write_all(&block).expect("Failed to write to file");



            }
            

        }
    }
    // Uncomment this block to pass the first stage
    // let args: Vec<String> = env::args().collect();
    // if args[1] == "init" {
    //     fs::create_dir(".git").unwrap();
    //     fs::create_dir(".git/objects").unwrap();
    //     fs::create_dir(".git/refs").unwrap();
    //     fs::write(".git/HEAD", "ref: refs/heads/main\n").unwrap();
    //     println!("Initialized git directory")
    // } else {
    //     println!("unknown command: {}", args[1])
    // }
}
