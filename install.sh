# Builds and Installs nvFanService to .local/bin
# might fail, idk, whatevs
cargo update
cargo build -r
cp target/release/nvfanservice ~/.local/bin
