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


