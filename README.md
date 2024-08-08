# Round Eliminator: a tool for automatic speedup simulation

Try it [here](https://roundeliminator.github.io/re-experimental/).

The old version is [here](https://roundeliminator.github.io/re/), and its code is [here](https://github.com/olidennis/round-eliminator/tree/round-eliminator-1).

The documentation is very outdated, but it can be found [here](https://olidennis.github.io/files/roundeliminatortutorial.pdf).

The author wishes to acknowledge CSC – IT Center for Science, Finland, for computational resources.

# If you want to run it on your machine (it is much faster compared to the wasm version)
## On Linux (Ubuntu)
First, install the dependencies:
```
sudo apt install curl git build-essential pkg-config libssl-dev cmake
```
Then, install rust by following the instructions [here](https://www.rust-lang.org/tools/install). Make sure to install the nightly version, by pressing 2 on "2) Customize installation" and typing "nightly".

Then, clone this repository:
```
git clone https://github.com/olidennis/round-eliminator.git
```

Then, run the server as follows:
```
cd round-eliminator/
cd round-eliminator-server
cargo run --release
```
Now, visit the url [http://127.0.0.1:8080/server](http://127.0.0.1:8080/server).

## On MacOS
Follow Linux instructions, use brew to install dependencies. [TODO: add more details]

## On Windows
It works. [TODO: add more details]

# Using Round Eliminator as a library
First, add the following line in your dependencies in Cargo.toml:
```
round-eliminator-lib = { git = "https://github.com/suomela/round-eliminator.git", branch = "current", version = "0.1.0" }
```
Then, add the following at the end of Cargo.toml:
```
[target.'cfg(not(target_env = "msvc"))'.dependencies]
tikv-jemallocator = "0.6"
```

Then, add the following at the beginning of main.rs:
```
#[cfg(not(target_env = "msvc"))]
use tikv_jemallocator::Jemalloc;

#[cfg(not(target_env = "msvc"))]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;
```

Note: Jemalloc not only makes round eliminator 30% faster, but it seems to also fix an issue on MacOS. More in detail,
without Jemalloc, on MacOS, on ARM CPUs, you may get random crashes, something like:
```
round-eliminator-server(75480,0x16cc4f000) malloc: *** error for object 0x60003ce07ff0: pointer being freed was not allocated
round-eliminator-server(75480,0x16cc4f000) malloc: *** set a breakpoint in malloc_error_break to debug
```
This seems to be related to some broken malloc implementation in the library included by Rust on MacOS, see [here](https://github.com/rust-lang/rust/issues/92173) and [here](https://users.rust-lang.org/t/intermittent-free-without-malloc-in-heavily-threaded-safe-code-on-arm64-mac/105154/3). Using Jemalloc seems to fix this issue.


# If you want to use Round Eliminator as a benchmark tool
After cloning the repository, do the following:
```
cd round-eliminator/
git reset --hard ea5bc43f44947f2614028d3e49d4de637ef7efbe
cd round-eliminator-benchmark/
cargo run --release
```

Results:
| CPU          | Single Thread Score | Multi Thread Score |
|--------------|---------------------|--------------------|
| Apple M1 Pro | 1553                | 12518              |

If you have a different CPU and you have benchmarked it, please send me the results!


