对于[课程](https://github.com/pingcap/talent-plan/blob/master/courses/rust/projects/project-1/README.md)的自我实践

本机环境：
```shell
~ » rustc -V
rustc 1.59.0 (9d1b2106e 2022-02-23)
```

目标: 通过命令行参数与保存在内存中的 K/V 存储打交道 

# Introduction
In this project you will create a simple in-memory key/value store that maps strings to strings, and that passes some tests and responds to command line arguments

The focus of this project is on the tooling and setup that goes into a **typical Rust project**.

# Project spec - 项目规范
要构件的是一个名叫 `kvs`的 client 工具

需要支持以下 arguments：
-   `kvs set <KEY> <VALUE>`: Set the value of a string key to a string
-   `kvs get <KEY>`:  Get the **string** value of a given string key
-   `kvs rm <KEY>`: Remove a given key
-   `kvs -V`: Print the version

`kvs` library 包含一个 type: `KvStore`, 其需要支持以下方法：
- `KvStore::set(&mut self, key: String, value: String)`: Set the value of a string key to a string
- `KvStore::get(&self, key: String) -> Option<String>`: Get the string value of the a string key. If the key does not exist, return `None`.
- `KvStore::remove(&mut self, key: String)`: Remove a given key.

将值保存在**内存**里。

The `get`/ `set` / `rm` commands will return an "unimplemented" error when run from the command line. 

Future projects will store values on disk and have a working command line interface.

# 初始化项目
```shell
cargo new kvs --lib
```

注意，上面的命令直接生成的不会是下面的结构，自己添加成下面的结构
```shell
$ tree kvs
kvs
├── Cargo.lock
├── Cargo.toml
├── src
│   ├── bin
│   │   └── kvs.rs
│   └── lib.rs
└── tests
    └── tests.rs
```

上面的`src/bin/kvs.rs`为避免报错，可以先写成：
```rust
fn main() {
    println!("Hello, world!");
}
```

`src/lib/rs`置空即可

`tests/`目录从[这里](https://github.com/pingcap/talent-plan/tree/master/courses/rust/projects/project-1/tests)搞过来。（TDD）

维护信息
```shell
~/rustpdrust/kvs(master*) » cat Cargo.toml
[package]
name = "kvs"
version = "0.1.0"
edition = "2021"
description = "A key-value store"
authors = ["pillipanda <pengsixiong@gmail.com>"]

[dependencies]
```

# part 1: make the test compile
虽然现在有了 unit test 文件，但是是并不能运行`cargo test`的

`rustc`即使在发现了 error 的情况下也会继续尝试编译，所以你能够看到**所有报错**！
```shell
~/rustpdrust/kvs(master*) » cargo test
   Compiling kvs v0.1.0 (/Users/pd/rustpdrust/kvs)
error[E0433]: failed to resolve: use of undeclared crate or module `assert_cmd`
 --> tests/tests.rs:1:5
  |
1 | use assert_cmd::prelude::*;
  |     ^^^^^^^^^^ use of undeclared crate or module `assert_cmd`

error[E0433]: failed to resolve: use of undeclared crate or module `predicates`
 --> tests/tests.rs:3:5
  |
3 | use predicates::str::contains;
  |     ^^^^^^^^^^ use of undeclared crate or module `predicates`
...
```

好吧，这里的 assert_cmd 与 predicates 都是外部依赖包。所以在 Cargo.toml 中添加：
```toml
[dev-dependencies]
assert_cmd = "0.11.0"
predicates = "1.0.0"
```

#important 那这里是如何发现这个报错是和外部依赖相关的呢？
```
```
1 | use assert_cmd::prelude::*;
  |     ^^^^^^^^^^ use of undeclared type or module `assert_cmd`
```
```

因为在开头被引入了呀！唉

ok，在添加了上面的`dev-dependencies`后再来运行：
```shell
~/rustpdrust/kvs(master*) » cargo test
   Compiling kvs v0.1.0 (/Users/pd/rustpdrust/kvs)
error[E0432]: unresolved import `kvs::KvStore`
 --> tests/tests.rs:2:5
  |
2 | use kvs::KvStore;
  |     ^^^^^^^^^^^^ no `KvStore` in the root

For more information about this error, try `rustc --explain E0432`.
error: could not compile `kvs` due to previous error
```

我们可以看到！只有一个报错了，而且就是需要你实现的 code 相关的。

现在来通过在 `src/lib.rs` 中编写相关的 type、method 定义(不写任何的 body)来使得命令`cargo test --no-run`编译成功吧！
![tdd_skeleton](/static/tdd_skeleton.png)

```rust
pub struct KvStore {}

impl KvStore {
    pub fn new() -> KvStore {
        KvStore{}
    }

    pub fn set(&mut self, key: String, value: String) -> () {
    }


    pub fn get(&self, key: String) -> Option<String> {
        Some(key)
    }

    pub fn remove(&mut self, key: String) {
    }
}
```

# Part 2: Accept command line arguments
The key / value stores throughout this course are all controlled through a command-line client. In this project the command-line client is very simple because the state of the key-value store is only stored in memory, not persisted to disk.

the interface for the CLI is:
- `kvs set <KEY> <VALUE>`：Set the value of a string key to a string
- `kvs get <KEY>`: Get the string value of a given string key
- `kvs rm <KEY>`: Remove a given key
- `kvs -V`: Print the version

目前的实现，get/set 命令会直接向 stderr 输出 string - "unimplemented" 并且exiting with a non-zero exit code, indicating an error.

你应该使用 `clap` 来处理 command-line arguments.

src/bin/kvs.rs:
```rust
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[clap(name = "kvs")]
#[clap(about = "A key-value store cli", long_about = None)]
#[clap(version)]  // 引入version会自动添加一个-V的OPTIONS，访问的话会自动输出Cargo.toml里的version
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
    let args = Cli::parse();

    match args.command {
        Commands::Get { key } => {
            eprintln!("get {} unimplemented", key)
        }
        Commands::Set { key, value } => {
            eprintln!("set {} {} unimplemented", key, value)
        }
        Commands::Rm { key } => {
            eprintln!("rm {} unimplemented", key)
        }
    }
}
```

看看使用：
```shell
~/rustpdrust/kvs(master*) » cargo run -- -h
kvs 0.1.0
A key-value store cli

USAGE:
    kvs <SUBCOMMAND>

OPTIONS:
    -h, --help       Print help information
    -V, --version    Print version information

SUBCOMMANDS:
    get     get value by key
    help    Print this message or the help of the given subcommand(s)
    rm      remove key and value
    set     set value to key
------------------------------------------------------------
~/rustpdrust/kvs(master*) » cargo run -- -V
kvs 0.1.0
------------------------------------------------------------
~/rustpdrust/kvs(master*) » cargo run -- set name pd
set name pd
------------------------------------------------------------
~/rustpdrust/kvs(master*) » cargo run -- get name
get name
~/rustpdrust/kvs(master*) » cargo run -- rm name 
rm name
```

# Part 4: Store values in memory
到此，项目的骨架已经搭建完成。

现在来实现那些方法里的 body 吧。所有需要实现的功能都被测试用例完备的定义了，使用 TDD 来实现吧！

src/bin/kvs.rs:
```rust
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
```

src/lib.rs
```rust
use std::collections::HashMap;

pub struct KvStore {
    kv_map: HashMap<String, String>,
}

impl KvStore {
    pub fn new() -> KvStore {
        KvStore {
            kv_map: HashMap::new(),
        }
    }

    pub fn set(&mut self, key: String, value: String) -> () {
        self.kv_map.insert(key, value);
        return;
    }

    pub fn get(&self, key: String) -> Option<String> {
        match self.kv_map.get(&key) {
            Some(value) => return Some(value.clone()),
            _ => return None,
        }
    }

    pub fn remove(&mut self, key: String) {
        self.kv_map.remove(&key);
    }
}
```

有了上面的东西后，可以通过目前的测试了：
```shell
~/rustpdrust/kvs(master*) » cargo test
   Compiling kvs v0.1.0 (/Users/pd/rustpdrust/kvs)
...
running 13 tests
test cli_get ... ok
test cli_invalid_get ... ok
test cli_invalid_rm ... ok
test cli_invalid_subcommand ... ok
test cli_invalid_set ... ok
test cli_rm ... ok
test cli_no_args ... ok
test cli_set ... ok
test get_non_existent_value ... ok
test get_stored_value ... ok
test overwrite_value ... ok
test remove_key ... ok
test cli_version ... ok

test result: ok. 13 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.10s

   Doc-tests kvs

running 0 tests

test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s
```

# Part 5: Documentation
先为 lib 中 pub 的代码部分添加文档注释。注释这里我直接用了[官方示例代码的注释](https://github.com/pingcap/talent-plan/blob/master/courses/rust/projects/project-1/src/kv.rs)

可以通过命令`cargo doc`进行生成文档的 html 相关文件并放到 target/doc 目录下。

可以通过命令`cargo doc --open`在浏览器中打开。

# Part 6: Ensure good style with clippy and rustfmt
- **clippy** helps ensure that code uses modern idioms, and prevents patterns that commonly lead to errors. 
- **rustfmt** enforces that code is formatted consistently. It's not necessary right now, but you might click those links and read their documentation. They are both sophisticated tools capable of much more than described below.

## clippy
运行`cargo clippy`:
```shell
~/rustpdrust/kvs(master*) » cargo clippy
    Checking kvs v0.1.0 (/Users/pd/rustpdrust/kvs)
warning: unneeded unit return type
  --> src/lib.rs:32:54
   |
32 |     pub fn set(&mut self, key: String, value: String) -> () {
   |                                                      ^^^^^^ help: remove the `-> ()`
   |
   = note: `#[warn(clippy::unused_unit)]` on by default
   = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#unused_unit

warning: unneeded `return` statement
  --> src/lib.rs:34:9
   |
34 |         return;
   |         ^^^^^^^ help: remove `return`
   |
   = note: `#[warn(clippy::needless_return)]` on by default
   = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#needless_return

warning: unneeded `return` statement
  --> src/lib.rs:42:28
   |
42 |             Some(value) => return Some(value.clone()),
   |                            ^^^^^^^^^^^^^^^^^^^^^^^^^^ help: remove `return`: `Some(value.clone())`
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#needless_return

warning: unneeded `return` statement
  --> src/lib.rs:43:18
   |
43 |             _ => return None,
   |                  ^^^^^^^^^^^ help: remove `return`: `None`
   |
   = help: for further information visit https://rust-lang.github.io/rust-clippy/master/index.html#needless_return
...
warning: `kvs` (bin "kvs") generated 4 warnings
    Finished dev [unoptimized + debuginfo] target(s) in 1.73s
```
上面只展示lib.rs里面的`clippy`给我的提示。可见存在一些冗余的代码需要进行清理

清理后的代码：
src/lib.rs
```rust
use std::collections::HashMap;

/// The `KvStore` stores string key/value pairs.
///
/// Key/value pairs are stored in a `HashMap` in memory and not persisted to disk.
///
/// Example:
///
/// ```rust
/// # use kvs::KvStore;
/// let mut store = KvStore::new();
/// store.set("key".to_owned(), "value".to_owned());
/// let val = store.get("key".to_owned());
/// assert_eq!(val, Some("value".to_owned()));
/// ```
#[derive(Default)]
pub struct KvStore {
    kv_map: HashMap<String, String>,
}

impl KvStore {
    /// Creates a `KvStore`.
    pub fn new() -> KvStore {
        KvStore {
            kv_map: HashMap::new(),
        }
    }

    /// Sets the value of a string key to a string.
    ///
    /// If the key already exists, the previous value will be overwritten.
    pub fn set(&mut self, key: String, value: String) {
        self.kv_map.insert(key, value);
    }

    /// Gets the string value of a given string key.
    ///
    /// Returns `None` if the given key does not exist.
    pub fn get(&self, key: String) -> Option<String> {
        if let Some(value) = self.kv_map.get(&key) {
            return Some(value.clone())
        }
        None
    }

    /// Remove a given string key.
    pub fn remove(&mut self, key: String) {
        self.kv_map.remove(&key);
    }
}
```

## rustfmt
运行`cargo fmt`

这个没啥可说的，就是 format 代码
