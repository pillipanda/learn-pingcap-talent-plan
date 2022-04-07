use clap::{Parser, Subcommand};
// use kvs::KvStore;
use std::process::exit;

#[derive(Debug, Parser)]
#[clap(name = "kvs")]
#[clap(about = "A key-value store cli", long_about = None)]
#[clap(version)] // 引入version会自动添加一个-V的OPTIONS，访问的话会自动输出Cargo.toml里的version
struct Cli {
    #[clap(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    /// get value by key
    #[clap(arg_required_else_help = true)]
    Get {
        /// The key
        key: String,
    },

    /// set value to key
    #[clap(arg_required_else_help = true)]
    Set {
        /// The string key
        key: String,
        /// The string value
        value: String,
    },

    /// remove key and value
    #[clap(arg_required_else_help = true)]
    Rm {
        /// The string key
        key: String,
    },
}

fn main() {
    // let mut kv_store = KvStore::new();
    let args = Cli::parse();

    match args.command {
        Commands::Get { key } => {
            eprintln!("unimplemented");
            exit(1);
            // match kv_store.get(key) {
            //     Some(val) => println!("{}", val),
            //     _ => println!(""),
            // }
        }
        Commands::Set { key, value } => {
            eprintln!("unimplemented");
            exit(1);
            // kv_store.set(key, value);
            // println!("OK")
        }
        Commands::Rm { key } => {
            eprintln!("unimplemented");
            exit(1);
            // kv_store.remove(key)
        }
    }
}
