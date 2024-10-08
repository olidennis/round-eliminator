# Round Eliminator: a tool for automatic speedup simulation

Round elimination is a technique for proving lower bounds on the distributed round complexity of locally checkable problems. For more info, see [this paper](https://arxiv.org/abs/1902.09958). Round Eliminator is a tool that allows to apply the round elimination technique automatically.

You can try it [here](https://roundeliminator.github.io/re-experimental/).

The old version is [here](https://roundeliminator.github.io/re/), and its code is [here](https://github.com/olidennis/round-eliminator/tree/round-eliminator-1).

The documentation is very outdated, but it can be found [here](https://olidennis.github.io/files/roundeliminatortutorial.pdf).

The author wishes to acknowledge CSC – IT Center for Science, Finland, for computational resources.

## How to run it on your machine 
Note: the link above points to the wasm version, which runs directly in the browser and is single-threaded. If you want Round Eliminator to be significantly faster, please run it on your machine, by following the instructions below.

### Precompiled binaries
The easier option is to just run the precompiled binaries.
Download [round-eliminator-server.zip](https://roundeliminator.github.io/releases/round-eliminator-server.zip). Unpack it. Move to round-eliminator-server/bin/ and run the appropriate binary (currently, the archive contains binaries for MacOS on Apple Silicon, Windows on x64, and Linux on x64). 
Then, visit the url [http://127.0.0.1:8080/server](http://127.0.0.1:8080/server).

### Compile On Linux (Ubuntu)
If you prefer to compile Round Eliminator yourself, first, install the dependencies:
```
sudo apt install curl git build-essential pkg-config libssl-dev cmake
```
Then, install rust by following the instructions [here](https://www.rust-lang.org/tools/install).

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


If you want Round Eliminator to be roughly 25% faster, you can use profile guided optimization, as follows.
```
cd round-eliminator/
cd round-eliminator-server
rustup component add llvm-tools-preview
cargo install cargo-pgo
RUSTFLAGS="-Ctarget-cpu=native" cargo pgo test pgo_quick_test
RUSTFLAGS="-Ctarget-cpu=native" cargo pgo optimize run
```

### Compile On MacOS
Follow Linux instructions, use brew to install dependencies. [TODO: add more details]

### Compile On Windows
It works. [TODO: add more details]

## How to use Round Eliminator as a library
First, add the following line to the dependencies section of Cargo.toml:
```
round-eliminator-lib = { git = "https://github.com/olidennis/round-eliminator.git", branch = "master", version = "0.1.0" }
```
Then, add the following to Cargo.toml:
```
[target.'cfg(not(target_os = "linux"))'.dependencies]
mimalloc = "0.1.43"
libmimalloc-sys = {version="0.1.39", features=["extended"]}

[target.'cfg(target_os = "linux")'.dependencies]
tikv-jemallocator = "0.6"
```

Then, add the following at the beginning of main.rs:
```
#[cfg(not(target_os = "linux"))]
use mimalloc::MiMalloc;
#[cfg(not(target_os = "linux"))]
#[global_allocator]
static GLOBAL: MiMalloc = MiMalloc;

#[cfg(target_os = "linux")]
use tikv_jemallocator::Jemalloc;
#[cfg(target_os = "linux")]
#[global_allocator]
static GLOBAL: Jemalloc = Jemalloc;
```
Then, in your main function, add the following:
```
    #[cfg(not(target_os = "linux"))]
    unsafe{ libmimalloc_sys::mi_option_set(26, 0) }
```
These two lines fix some issue introduced in MiMalloc 2.1.4 on Windows. In particular, these lines disable the option `mi_option_abandoned_reclaim_on_free`, which on Windows seems to drastically increase the memory usage (while using the automatic upper bound feature of round eliminator, the memory usage with the option disabled is in the ballpark of 50MB, but with the option enabled it quickly uses all the memory of the machine).

Note: Mimalloc and Jemalloc make Round Eliminator roughly 30% faster. Mimalloc seems to be better on Windows and MacOS, while Jemalloc seems to be better on Linux. Moreover, not using the default allocator seems to also fix an issue on MacOS. More in detail, without Mimalloc, on MacOS, on ARM CPUs, you may get random crashes, something like:
```
round-eliminator-server(75480,0x16cc4f000) malloc: *** error for object 0x60003ce07ff0: pointer being freed was not allocated
round-eliminator-server(75480,0x16cc4f000) malloc: *** set a breakpoint in malloc_error_break to debug
```
This seems to be related to some broken malloc implementation in the library included by Rust on MacOS, see [here](https://github.com/rust-lang/rust/issues/92173) and [here](https://users.rust-lang.org/t/intermittent-free-without-malloc-in-heavily-threaded-safe-code-on-arm64-mac/105154/3). Using Mimalloc seems to fix this issue.


If you want to use profile guided optimization, you can add the following to your main.rs file:
```
#[test]
fn pgo_quick_test() {               
    assert!(std::hint::black_box(round_eliminator_lib::test_all_short()) > 0);
}
```
Then, use the following commands to run your code:
```
rustup component add llvm-tools-preview
cargo install cargo-pgo
RUSTFLAGS="-Ctarget-cpu=native" cargo pgo test pgo_quick_test
RUSTFLAGS="-Ctarget-cpu=native" cargo pgo optimize run
```

## How to use Round Eliminator as a benchmark tool/stress test

You can find the precompiled binaries here:
| Platform | Link |
|--------------------------|-----------|
| MacOS on Apple Silicon | [here](https://roundeliminator.github.io/releases/round-eliminator-benchmark_2.0.2_aarch64_macos) |
| Linux on x64           | [here](https://roundeliminator.github.io/releases/round-eliminator-benchmark_2.0.2_x64_linux) |
| Windows on x64 | [here](https://roundeliminator.github.io/releases/round-eliminator-benchmark_2.0.2_x64_windows.exe) |

Otherwise, to compile it yourself, follow these instructions.
After cloning the repository, do the following:
```
cd round-eliminator/
git reset --hard 4fb8c59393d54bc5fccf8c898a500c60764f64dd
cd round-eliminator-benchmark/
rustup component add llvm-tools-preview
cargo install cargo-pgo
```
Run the following command:
```
cargo pgo test pgo_quick_test
```
Run the following to get the multi thread score:
```
cargo pgo optimize run -- -- -m
```
Run the following to get the single thread score:
```
cargo pgo optimize run -- -- -s
```

Results:
| CPU          | OS | Single Thread Score | Multi Thread Score |
|--------------|----|-----------------|--------------------|
| AMD Ryzen 7 7800X3D | Ubuntu 24.04 | 2731 | 25265   |
| AMD Ryzen 7 7800X3D | Windows 11 | 2613 | 25026   |
| Apple M1 Pro | MacOS 14.5 | 2216  | 17647         |





