# Round Eliminator: a tool for automatic speedup simulation

Round elimination is a technique for proving lower bounds on the distributed round complexity of locally checkable problems. For more info, see [this paper](https://arxiv.org/abs/1902.09958). Round eliminator is a tool that allows to apply the round elimination technique automatically.

You can try it [here](https://roundeliminator.github.io/re-experimental/).

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


If you want round eliminator to be roughly 25% faster, you can use profile guided optimization, as follows.
```
cd round-eliminator/
cd round-eliminator-server
rustup component add llvm-tools-preview
cargo install cargo-pgo
RUSTFLAGS="-Ctarget-cpu=native" cargo pgo test benchmark
RUSTFLAGS="-Ctarget-cpu=native" cargo pgo optimize run
```

Otherwise, you can find the precompiled binaries here:
| Platform | Link |
|--------------------------|-----------|
| MacOS with Apple Silicon | [here](https://roundeliminator.github.io/releases/round-eliminator-server_2.0.0_aarch64-apple-darwin) |
| Linux on x64             | TODO                                                                                                     |
| Windows on x64           | TODO                                                                                                     |


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

You can find the precompiled binaries here:
| Platform | Link |
|--------------------------|-----------|
| MacOS with Apple Silicon | [here](https://roundeliminator.github.io/releases/round-eliminator-benchmark_2.0.0_aarch64-apple-darwin) |
| Linux on x64             | TODO                                                                                                     |
| Windows on x64           | TODO                                                                                                     |

Otherwise, to compile it yourself, follow these instructions.
After cloning the repository, do the following:
```
cd round-eliminator/
git reset --hard 0884823bd343b5a2d1011cf3dfd4c43d0db34c18
cd round-eliminator-benchmark/
rustup component add llvm-tools-preview
cargo install cargo-pgo
```
The following command will take a lot of time:
```
RUSTFLAGS="-Ctarget-cpu=native" cargo pgo run -- -- -m -d
```
Run the following to get the multi thread score:
```
RUSTFLAGS="-Ctarget-cpu=native" cargo pgo optimize run -- -- -m
```
Run the following to get the single thread score:
```
RUSTFLAGS="-Ctarget-cpu=native" cargo pgo optimize run -- -- -s
```

Results:
| CPU          | Single Thread Score | Multi Thread Score |
|--------------|---------------------|--------------------|
| Apple M1 Pro | 2069                | 16367              |

If you have a different CPU and you have benchmarked it, please send me the results!


