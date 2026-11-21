These benchmarks were measured on 2026-11-20, with ripcloc built at `aeb7c76` by `cargo build --release`, through
`bench/suite` from the root of this repository:

    $ bench/suite --dir bench/corpora --download
    $ bench/suite --dir bench/corpora \
          --raw bench/runs/2026-11-20-ubuntu2604-fuzz-aeb7c76/raw.csv \
          > bench/runs/2026-11-20-ubuntu2604-fuzz-aeb7c76/summary

The machine is a KVM virtual machine with 16 dedicated vCPUs of an AMD EPYC (Milan) processor, as 8 cores of 2 threads, and
61 GB of memory, running Ubuntu 26.04 with Linux 7.0. Every tool was pinned to vCPUs 1 to 15. The versions of the
tools and the commit of every corpus are at the top of `summary`.

    ripcloc 1.0.0
    tokei 15.0.0
    scc 4.1.0
    cloc 2.10

Each tool other than cloc has ten timed runs after three rounds of warmup, and three runs through `/usr/bin/time` for
its peak memory. cloc has one run, which is timed and measured at once.

A short run of one thread is bimodal on this machine, depending on which core it starts on, so the benchmarks of one
file have a wider deviation than the others, for every tool. ripcloc's peak memory varies between runs on the largest
corpora, from 56 to 110 MiB on linux, since each walker thread keeps a read buffer as large as the largest file that it
has read, up to 16 MiB. Even its largest run used less memory than the smallest run of every other tool.

`bench/suite --markdown bench/runs/2026-11-20-ubuntu2604-fuzz-aeb7c76/raw.csv` prints these results as tables, with
each tool's multiples of ripcloc combined by their geometric mean.
